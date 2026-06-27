use std::collections::HashMap;

use gpui::{Bounds, Pixels};
use serde::Serialize;

use crate::app_state::{DetailTab, GitMasterApp};

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

impl GitMasterApp {
    pub fn build_view_tree(&self) -> ViewNode {
        let reg = self.bounds_registry.lock().ok();
        let empty = HashMap::new();
        let reg = reg.as_deref().unwrap_or(&empty);

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
            bar = bar.with_child(ViewNode::new("text").with_id("status-message").with_text(msg));
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
            .map(|(i, repo)| {
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
                    .with_child(ViewNode::new("text").with_text(&repo.current_branch))
                    .with_child(ViewNode::new("text").with_text(dirty_text));

                if let Some(ab) = ab {
                    item = item.with_child(ViewNode::new("text").with_text(&ab));
                }

                item
            })
            .collect();

        list.with_children(items)
    }

    fn build_detail_panel_node(
        &self,
        reg: &HashMap<String, Bounds<Pixels>>,
    ) -> Option<ViewNode> {
        self.selected_index?;

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
                ViewNode::new("label")
                    .with_text(&format!("Branch: {}", detail.current_branch)),
            )
            .with_child(ViewNode::new("label").with_text(&format!(
                "Remote: {}",
                detail.remote_url.as_deref().unwrap_or("(none)")
            )))
            .with_child(
                ViewNode::new("label")
                    .with_text(&format!("File Status: {status_text}")),
            )
    }

    fn build_log_node(&self) -> ViewNode {
        let entries: Vec<ViewNode> = self
            .log_entries
            .iter()
            .map(|entry| {
                ViewNode::new("list-item")
                    .with_child(ViewNode::new("text").with_text(&entry.hash))
                    .with_child(ViewNode::new("text").with_text(&entry.message))
                    .with_child(
                        ViewNode::new("text")
                            .with_text(&format!("{} — {}", entry.author, entry.date)),
                    )
            })
            .collect();

        ViewNode::new("panel")
            .with_id("log-content")
            .with_children(entries)
    }

    fn build_context_menu_node(
        &self,
        reg: &HashMap<String, Bounds<Pixels>>,
    ) -> Option<ViewNode> {
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
