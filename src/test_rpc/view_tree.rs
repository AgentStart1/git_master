use std::collections::HashMap;

use gpui::{Bounds, Pixels};
use serde::Serialize;

use crate::app_state::{ContextMenu, DetailTab, GitMasterApp, RepoSelection};
use crate::models::{LogEntry, RepoDetail, RepoInfo, SubmoduleDetail};
use crate::ui::commit_canvas::{CanvasEdgeKind, CommitCanvasLayout, LogViewMode};

#[derive(Serialize, Clone, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    fn from_gpui(bounds: &Bounds<Pixels>) -> Self {
        Self {
            x: bounds.origin.x.into(),
            y: bounds.origin.y.into(),
            width: bounds.size.width.into(),
            height: bounds.size.height.into(),
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ViewNode {
    pub node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub interactive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Rect>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ViewNode>,
}

impl ViewNode {
    fn new(node_type: &str) -> Self {
        Self {
            node_type: node_type.to_string(),
            id: None,
            text: None,
            interactive: false,
            bounds: None,
            children: Vec::new(),
        }
    }

    fn with_id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    fn with_text(mut self, text: &str) -> Self {
        self.text = Some(text.to_string());
        self
    }

    fn with_interactive(mut self) -> Self {
        self.interactive = true;
        self
    }

    fn with_bounds_from(mut self, registry: &HashMap<String, Bounds<Pixels>>, id: &str) -> Self {
        if let Some(b) = registry.get(id) {
            self.bounds = Some(Rect::from_gpui(b));
        }
        self
    }

    fn with_child(mut self, child: ViewNode) -> Self {
        self.children.push(child);
        self
    }

    fn with_children(mut self, children: Vec<ViewNode>) -> Self {
        self.children.extend(children);
        self
    }
}

pub struct TestViewTreeSnapshot {
    bounds: HashMap<String, Bounds<Pixels>>,
    parent_dir: Option<std::path::PathBuf>,
    repos: Vec<RepoInfo>,
    selected: Option<RepoSelection>,
    expanded_repos: std::collections::BTreeSet<usize>,
    active_tab: DetailTab,
    detail: Option<RepoDetail>,
    submodule_detail: Option<SubmoduleDetail>,
    log_entries: Vec<LogEntry>,
    log_view_mode: LogViewMode,
    commit_canvas_layout: Option<CommitCanvasLayout>,
    scanning: bool,
    loading_detail: bool,
    context_menu: Option<ContextMenu>,
    status_message: Option<String>,
}

impl GitMasterApp {
    pub fn test_view_tree_snapshot(&self) -> TestViewTreeSnapshot {
        TestViewTreeSnapshot {
            bounds: self.bounds_registry.borrow().clone(),
            parent_dir: self.parent_dir.clone(),
            repos: self.repos.clone(),
            selected: self.selected.clone(),
            expanded_repos: self.expanded_repos.clone(),
            active_tab: self.active_tab,
            detail: self.detail.clone(),
            submodule_detail: self.submodule_detail.clone(),
            log_entries: self.log_entries.clone(),
            log_view_mode: self.log_view_mode,
            commit_canvas_layout: self.commit_canvas_layout.clone(),
            scanning: self.scanning,
            loading_detail: self.loading_detail,
            context_menu: self.context_menu.clone(),
            status_message: self.status_message.clone(),
        }
    }
}

impl TestViewTreeSnapshot {
    pub fn build(&self) -> ViewNode {
        let reg = &self.bounds;

        let top_bar = self.build_top_bar_node(reg);
        let repo_list = self.build_repo_list_node(reg);
        let detail_panel = self.build_detail_panel_node(reg);

        let mut main_content = ViewNode::new("panel")
            .with_id("main-content")
            .with_bounds_from(reg, "main-content")
            .with_child(repo_list);

        if let Some(panel) = detail_panel {
            main_content = main_content.with_child(panel);
        }

        let mut root = ViewNode::new("window")
            .with_id("root")
            .with_bounds_from(reg, "root")
            .with_child(top_bar)
            .with_child(main_content);

        if let Some(menu) = self.build_context_menu_node(reg) {
            root = root.with_child(menu);
        }

        root
    }

    fn build_top_bar_node(&self, reg: &HashMap<String, Bounds<Pixels>>) -> ViewNode {
        let dir_label = self
            .parent_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "No directory selected".into());

        let mut bar = ViewNode::new("panel")
            .with_id("top-bar")
            .with_bounds_from(reg, "top-bar")
            .with_child(ViewNode::new("text").with_text(&dir_label));

        if let Some(msg) = &self.status_message {
            bar = bar.with_child(
                ViewNode::new("text")
                    .with_id("status-message")
                    .with_text(msg),
            );
        }

        bar = bar.with_child(
            ViewNode::new("button")
                .with_id("change-dir-btn")
                .with_text("Open Directory")
                .with_interactive()
                .with_bounds_from(reg, "change-dir-btn"),
        );

        bar
    }

    fn build_repo_list_node(&self, reg: &HashMap<String, Bounds<Pixels>>) -> ViewNode {
        let mut list = ViewNode::new("panel")
            .with_id("repo-list")
            .with_bounds_from(reg, "repo-list-panel");

        if self.scanning {
            list = list.with_child(ViewNode::new("text").with_text("Scanning…"));
        }

        let items: Vec<ViewNode> = self
            .repos
            .iter()
            .enumerate()
            .flat_map(|(i, repo)| {
                let id = format!("repo-{i}");
                let dirty_text = if repo.is_dirty { "●" } else { "✓" };
                let ab = if repo.ahead > 0 || repo.behind > 0 {
                    Some(format!("↑{} ↓{}", repo.ahead, repo.behind))
                } else {
                    None
                };

                let mut item = ViewNode::new("list-item")
                    .with_id(&id)
                    .with_interactive()
                    .with_bounds_from(reg, &id)
                    .with_child(ViewNode::new("text").with_text(&repo.name))
                    .with_child(
                        ViewNode::new("text")
                            .with_text(&format!("Branch: {}", repo.current_branch)),
                    )
                    .with_child(ViewNode::new("text").with_text(dirty_text));

                if let Some(ab) = ab {
                    item = item.with_child(ViewNode::new("text").with_text(&ab));
                }

                let mut items = vec![item];
                if self.expanded_repos.contains(&i) {
                    items.extend(repo.submodules.iter().enumerate().map(|(j, submodule)| {
                        let id = format!("repo-{i}-submodule-{j}");
                        let dirty_text = if submodule.is_dirty {
                            "●"
                        } else if submodule.is_initialized {
                            "✓"
                        } else {
                            "!"
                        };
                        let mut item = ViewNode::new("list-item")
                            .with_id(&id)
                            .with_interactive()
                            .with_bounds_from(reg, &id)
                            .with_child(ViewNode::new("text").with_text(&submodule.name))
                            .with_child(ViewNode::new("text").with_text(&submodule.current_branch))
                            .with_child(ViewNode::new("text").with_text(dirty_text));

                        if submodule.ahead > 0 || submodule.behind > 0 {
                            item =
                                item.with_child(ViewNode::new("text").with_text(&format!(
                                    "↑{} ↓{}",
                                    submodule.ahead, submodule.behind
                                )));
                        }
                        item
                    }));
                }

                items
            })
            .collect();

        list.with_children(items)
    }

    fn build_detail_panel_node(&self, reg: &HashMap<String, Bounds<Pixels>>) -> Option<ViewNode> {
        self.selected.as_ref()?;

        let info_tab = ViewNode::new("tab")
            .with_id("tab-info")
            .with_text("Info")
            .with_interactive()
            .with_bounds_from(reg, "tab-info");

        let log_tab = ViewNode::new("tab")
            .with_id("tab-log")
            .with_text("Git Log")
            .with_interactive()
            .with_bounds_from(reg, "tab-log");

        let tab_bar = ViewNode::new("panel")
            .with_id("tab-bar")
            .with_child(info_tab)
            .with_child(log_tab);

        let body = if self.loading_detail {
            ViewNode::new("text").with_text("Loading…")
        } else if let Some(submodule) = &self.submodule_detail
            && !submodule.is_initialized
        {
            self.build_uninitialized_submodule_node(submodule)
        } else if let Some(detail) = &self.detail {
            match self.active_tab {
                DetailTab::Info => self.build_info_node(detail),
                DetailTab::GitLog => self.build_log_node(),
            }
        } else {
            ViewNode::new("text").with_text("Failed to open repository.")
        };

        let panel = ViewNode::new("panel")
            .with_id("detail-panel")
            .with_bounds_from(reg, "detail-panel")
            .with_child(tab_bar)
            .with_child(
                ViewNode::new("panel")
                    .with_id("detail-content")
                    .with_child(body),
            );

        Some(panel)
    }

    fn build_info_node(&self, detail: &crate::models::RepoDetail) -> ViewNode {
        let status_text = format!(
            "{} new, {} modified, {} deleted, {} renamed, {} conflicted",
            detail.file_status.new_files,
            detail.file_status.modified,
            detail.file_status.deleted,
            detail.file_status.renamed,
            detail.file_status.conflicted,
        );

        ViewNode::new("panel")
            .with_id("info-content")
            .with_child(ViewNode::new("label").with_text(&format!("Path: {}", detail.path)))
            .with_child(
                ViewNode::new("label").with_text(&format!("Branch: {}", detail.current_branch)),
            )
            .with_child(ViewNode::new("label").with_text(&format!(
                "Remotes: {}",
                if detail.remotes.is_empty() {
                    "(none)".to_string()
                } else {
                    detail
                        .remotes
                        .iter()
                        .map(|remote| {
                            format!(
                                "{} ({})",
                                remote.name,
                                remote.url.as_deref().unwrap_or("no URL")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            )))
            .with_child(ViewNode::new("label").with_text(&format!("File Status: {status_text}")))
    }

    fn build_uninitialized_submodule_node(
        &self,
        detail: &crate::models::SubmoduleDetail,
    ) -> ViewNode {
        ViewNode::new("panel")
            .with_id("submodule-info-content")
            .with_child(ViewNode::new("label").with_text(&format!("Name: {}", detail.name)))
            .with_child(ViewNode::new("label").with_text(&format!("Path: {}", detail.path)))
            .with_child(ViewNode::new("label").with_text(&format!(
                "URL: {}",
                detail.url.as_deref().unwrap_or("(none)")
            )))
            .with_child(ViewNode::new("label").with_text("Status: Not initialized"))
            .with_child(
                ViewNode::new("button")
                    .with_id("init-submodule-btn")
                    .with_text("Initialize Submodule")
                    .with_interactive(),
            )
    }

    fn build_log_node(&self) -> ViewNode {
        let toolbar = ViewNode::new("panel")
            .with_id("log-view-toolbar")
            .with_child(
                ViewNode::new("button")
                    .with_id("log-view-list")
                    .with_text("List")
                    .with_interactive(),
            )
            .with_child(
                ViewNode::new("button")
                    .with_id("log-view-canvas")
                    .with_text("Canvas")
                    .with_interactive(),
            );

        let body = match self.log_view_mode {
            LogViewMode::List => self.build_log_list_node(),
            LogViewMode::Canvas => self.build_commit_canvas_node(),
        };

        ViewNode::new("panel")
            .with_id("log-content")
            .with_child(toolbar)
            .with_child(body)
    }

    fn build_log_list_node(&self) -> ViewNode {
        let entries: Vec<ViewNode> = self
            .log_entries
            .iter()
            .map(|entry| {
                ViewNode::new("list-item")
                    .with_child(ViewNode::new("text").with_text(&entry.hash))
                    .with_child(ViewNode::new("text").with_text(&entry.message))
                    .with_children(
                        self.detail
                            .as_ref()
                            .and_then(|detail| detail.head_labels.get(&entry.full_hash))
                            .into_iter()
                            .flatten()
                            .map(|label| ViewNode::new("label").with_text(label))
                            .collect(),
                    )
                    .with_child(
                        ViewNode::new("text")
                            .with_text(&format!("{} — {}", entry.author, entry.date)),
                    )
            })
            .collect();

        ViewNode::new("list")
            .with_id("log-list")
            .with_children(entries)
    }

    fn build_commit_canvas_node(&self) -> ViewNode {
        let Some(layout) = self.commit_canvas_layout.as_ref() else {
            return ViewNode::new("panel")
                .with_id("commit-canvas")
                .with_child(ViewNode::new("text").with_text("Commit graph is unavailable."));
        };
        let lanes = layout.lanes.iter().map(|lane| {
            ViewNode::new("group")
                .with_id(&format!("commit-lane-{}", lane.id))
                .with_text(&lane.name)
        });
        let nodes = layout.nodes.iter().map(|node| {
            let mut view = ViewNode::new("commit-node")
                .with_id(&format!(
                    "commit-node-{}-{}",
                    node.id.lane_id,
                    node.short_hash()
                ))
                .with_interactive()
                .with_child(ViewNode::new("text").with_text(node.short_hash()));
            if let Some(entry) = node.entry.as_ref() {
                view = view.with_child(ViewNode::new("text").with_text(&entry.message));
            } else {
                view = view
                    .with_child(ViewNode::new("text").with_text("Referenced commit not loaded"));
            }
            view
        });
        let edges = layout.edges.iter().map(|edge| {
            ViewNode::new("graph-edge").with_text(&format!(
                "{}:{} -> {}:{} ({})",
                edge.from.lane_id,
                &edge.from.commit_id[..7.min(edge.from.commit_id.len())],
                edge.to.lane_id,
                &edge.to.commit_id[..7.min(edge.to.commit_id.len())],
                match edge.kind {
                    CanvasEdgeKind::Parent => "parent",
                    CanvasEdgeKind::Submodule => "submodule",
                    CanvasEdgeKind::Head => "head",
                }
            ))
        });

        ViewNode::new("canvas")
            .with_id("commit-canvas")
            .with_interactive()
            .with_children(lanes.collect())
            .with_children(nodes.collect())
            .with_children(edges.collect())
    }

    fn build_context_menu_node(&self, reg: &HashMap<String, Bounds<Pixels>>) -> Option<ViewNode> {
        let menu = self.context_menu.as_ref()?;
        let current_branch = self
            .repos
            .get(menu.repo_index)
            .map(|r| r.current_branch.as_str())
            .unwrap_or_default();

        let mut node = ViewNode::new("menu")
            .with_id("context-menu")
            .with_bounds_from(reg, "context-menu")
            .with_child(
                ViewNode::new("menu-item")
                    .with_id("ctx-switch-branch")
                    .with_text(if menu.show_branches {
                        "▾ Switch Branch"
                    } else {
                        "▸ Switch Branch"
                    })
                    .with_interactive()
                    .with_bounds_from(reg, "ctx-switch-branch"),
            );

        if menu.show_branches {
            let branch_items: Vec<ViewNode> = menu
                .branches
                .iter()
                .filter(|b| b.as_str() != current_branch)
                .map(|b| {
                    let id = format!("ctx-branch-{b}");
                    ViewNode::new("menu-item")
                        .with_id(&id)
                        .with_text(b)
                        .with_interactive()
                        .with_bounds_from(reg, &id)
                })
                .collect();
            node = node.with_children(branch_items);
        }

        node = node
            .with_child(
                ViewNode::new("menu-item")
                    .with_id("ctx-pull-rebase")
                    .with_text("Pull --rebase")
                    .with_interactive()
                    .with_bounds_from(reg, "ctx-pull-rebase"),
            )
            .with_child(
                ViewNode::new("menu-item")
                    .with_id("ctx-push")
                    .with_text("Push")
                    .with_interactive()
                    .with_bounds_from(reg, "ctx-push"),
            );

        Some(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_builds_a_view_tree_without_accessing_live_ui_state() {
        let app = GitMasterApp::new();

        let tree = app.test_view_tree_snapshot().build();

        assert_eq!(tree.node_type, "window");
        assert_eq!(tree.id.as_deref(), Some("root"));
        assert!(!tree.children.is_empty());
    }
}
