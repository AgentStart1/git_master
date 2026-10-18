use std::collections::HashMap;

use gpui::{Bounds, Pixels};
use serde::Serialize;

use crate::app_state::{ContextMenu, DetailTab, GitMasterApp, RepoSelection};
use crate::models::{LogEntry, RepoDetail, RepoInfo, SubmoduleDetail};
use crate::ui::commit_canvas::{CanvasEdgeKind, CanvasNodeKind, CommitCanvasLayout, LogViewMode};

#[derive(Serialize, Clone, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub(super) fn from_gpui(bounds: &Bounds<Pixels>) -> Self {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<serde_json::Value>,
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
    pub(super) fn new(node_type: &str) -> Self {
        Self {
            state: None,
            node_type: node_type.to_string(),
            id: None,
            text: None,
            interactive: false,
            bounds: None,
            children: Vec::new(),
        }
    }

    pub(super) fn with_id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    pub(super) fn with_state(mut self, state: serde_json::Value) -> Self {
        self.state = Some(state);
        self
    }

    pub(super) fn with_text(mut self, text: &str) -> Self {
        self.text = Some(text.to_string());
        self
    }

    pub(super) fn with_interactive(mut self) -> Self {
        self.interactive = true;
        self
    }

    fn with_bounds_from(mut self, registry: &HashMap<String, Bounds<Pixels>>, id: &str) -> Self {
        if let Some(b) = registry.get(id) {
            self.bounds = Some(Rect::from_gpui(b));
        }
        self
    }

    pub(super) fn with_child(mut self, child: ViewNode) -> Self {
        self.children.push(child);
        self
    }

    fn with_children(mut self, children: Vec<ViewNode>) -> Self {
        self.children.extend(children);
        self
    }
}

pub struct TestViewTreeSnapshot {
    test_prompt: Option<super::prompt::Description>,
    commit_text: Option<Result<String, String>>,
    selected_commit: bool,
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
    status_expanded: bool,
    status_summary: Option<String>,
    status_revision: u64,
    view_revision: u64,
    worktree: Option<(
        std::path::PathBuf,
        Result<crate::worktree::Snapshot, String>,
    )>,
    worktree_loading: bool,
    busy: bool,
}

impl GitMasterApp {
    pub fn test_view_tree_snapshot(&self) -> TestViewTreeSnapshot {
        TestViewTreeSnapshot {
            test_prompt: self.test_prompt.borrow().clone(),
            commit_text: self.commit_text.clone(),
            selected_commit: self.selected_commit.is_some(),
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
            status_expanded: self.status_expanded,
            status_summary: self.task_status_summary(),
            status_revision: self.status_revision,
            view_revision: self.test_view_revision,
            worktree: self.worktree.clone(),
            worktree_loading: self.worktree_loading,
            busy: self.busy,
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

        if self.selected_commit {
            root = root.with_child(
                ViewNode::new("panel")
                    .with_id("commit-details")
                    .with_text(&match &self.commit_text {
                        Some(Ok(text)) => text.clone(),
                        Some(Err(error)) => error.clone(),
                        None => "Loading commit…".into(),
                    })
                    .with_child(
                        ViewNode::new("button")
                            .with_id("close-commit-details")
                            .with_text("Close")
                            .with_interactive(),
                    ),
            );
        }
        if let Some(prompt) = &self.test_prompt {
            root = root.with_child(
                ViewNode::new("dialog")
                    .with_id("test-prompt")
                    .with_text(&prompt.message)
                    .with_child(ViewNode::new("text").with_text(&prompt.detail))
                    .with_children(
                        prompt
                            .answers
                            .iter()
                            .enumerate()
                            .map(|(i, label)| {
                                ViewNode::new("button")
                                    .with_id(&format!("test-prompt-answer-{i}"))
                                    .with_text(label)
                                    .with_interactive()
                            })
                            .collect(),
                    ),
            );
        }
        fn attach_bounds(node: &mut ViewNode, reg: &HashMap<String, Bounds<Pixels>>) {
            if let Some(bounds) = node.id.as_ref().and_then(|id| reg.get(id)) {
                node.bounds = Some(Rect::from_gpui(bounds));
            }
            for child in &mut node.children {
                attach_bounds(child, reg);
            }
        }
        attach_bounds(&mut root, reg);
        root.state = Some(serde_json::json!({
            "busy": self.busy, "loading_detail": self.loading_detail,
            "status_revision": self.status_revision,
            "view_revision": self.view_revision,
            "scanning": self.scanning, "worktree_loading": self.worktree_loading,
            "path": self.detail.as_ref().map(|detail| &detail.path),
            "tab": match self.active_tab { DetailTab::Info => "info", DetailTab::GitLog => "log", DetailTab::GitChange => "change", DetailTab::GitStash => "stash" },
        }));
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
                .with_id("open-logs-btn")
                .with_text("Logs")
                .with_interactive(),
        );
        for id in ["status-toggle", "status-copy"] {
            if self.status_message.is_some() {
                bar = bar.with_child(
                    ViewNode::new("button")
                        .with_id(id)
                        .with_interactive()
                        .with_bounds_from(reg, id),
                );
            }
        }
        if let Some(summary) = &self.status_summary {
            bar = bar.with_child(
                ViewNode::new("text")
                    .with_id("status-summary")
                    .with_text(summary)
                    .with_bounds_from(reg, "status-summary"),
            );
        }
        if self.status_expanded {
            bar = bar.with_child(
                ViewNode::new("panel")
                    .with_id("status-details")
                    .with_interactive()
                    .with_text(self.status_message.as_deref().unwrap_or_default())
                    .with_bounds_from(reg, "status-details")
                    .with_child(
                        ViewNode::new("text")
                            .with_id("status-detail-end")
                            .with_text(
                                self.status_message
                                    .as_deref()
                                    .unwrap_or_default()
                                    .rsplit('\n')
                                    .next()
                                    .unwrap_or_default(),
                            )
                            .with_bounds_from(reg, "status-detail-end"),
                    ),
            );
        }

        bar = bar.with_child(
            ViewNode::new("button")
                .with_id("fetch-all-btn")
                .with_text("Fetch All")
                .with_interactive()
                .with_bounds_from(reg, "fetch-all-btn"),
        );
        bar = bar.with_child(
            ViewNode::new("button")
                .with_id("switch-all-main-btn")
                .with_text("Switch All to Main")
                .with_interactive()
                .with_bounds_from(reg, "switch-all-main-btn"),
        );
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
                if !repo.submodules.is_empty() {
                    item = item.with_child(
                        ViewNode::new("button")
                            .with_id(&format!("repo-{i}-toggle"))
                            .with_interactive(),
                    );
                }

                for status in &repo.remote_statuses {
                    item = item.with_child(ViewNode::new("text").with_text(&status.label()));
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
                        for status in &submodule.remote_statuses {
                            item =
                                item.with_child(ViewNode::new("text").with_text(&status.label()));
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
            .with_child(log_tab)
            .with_children(
                [("tab-change", "Git Change"), ("tab-stash", "Git Stash")]
                    .into_iter()
                    .map(|(id, label)| {
                        ViewNode::new("tab")
                            .with_id(id)
                            .with_text(label)
                            .with_interactive()
                            .with_bounds_from(reg, id)
                    })
                    .collect(),
            );

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
                DetailTab::GitChange => self.build_worktree_node(false, reg),
                DetailTab::GitStash => self.build_worktree_node(true, reg),
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

    fn build_worktree_node(&self, stash: bool, reg: &HashMap<String, Bounds<Pixels>>) -> ViewNode {
        let button = |id: &str, label: &str| {
            let node = ViewNode::new("button")
                .with_id(id)
                .with_text(label)
                .with_bounds_from(reg, id);
            if self.busy || self.worktree_loading {
                node
            } else {
                node.with_interactive()
            }
        };
        let mut node = ViewNode::new("panel")
            .with_id("worktree-content")
            .with_child(button("worktree-refresh", "Refresh"));
        if stash {
            node = node.with_child(button("stash-save", "Save stash"));
        }
        if self.worktree_loading {
            return node.with_child(ViewNode::new("text").with_text("Loading…"));
        }
        let Some((path, result)) = &self.worktree else {
            return node;
        };
        if self
            .detail
            .as_ref()
            .is_none_or(|detail| std::path::Path::new(&detail.path) != path)
        {
            return node;
        }
        let data = match result {
            Ok(data) => data,
            Err(error) => return node.with_child(ViewNode::new("text").with_text(error)),
        };
        if stash {
            if data.stashes.is_empty() {
                node = node.with_child(ViewNode::new("text").with_text("No stashes."));
            }
            for entry in &data.stashes {
                let id = entry.oid;
                node = node.with_child(
                    ViewNode::new("text")
                        .with_text(&format!("stash@{{{}}} {}", entry.index, entry.message)),
                );
                node = node.with_child(button(
                    &format!("stash-select-{id}"),
                    &format!("stash@{{{}}} {}", entry.index, entry.message),
                ));
                for (action, label) in [
                    ("show", "View"),
                    ("apply", "Apply"),
                    ("pop", "Pop"),
                    ("drop", "Delete"),
                ] {
                    node = node.with_child(button(&format!("stash-{action}-{id}"), label));
                }
            }
        } else {
            if data.changes.is_empty() {
                node = node.with_child(ViewNode::new("text").with_text("Working tree clean."));
            }
            for (index, entry) in data.changes.iter().enumerate() {
                node = node
                    .with_child(ViewNode::new("text").with_text(&format!(
                        "{} — {}",
                        entry.path.display(),
                        entry.status
                    )).with_state(serde_json::json!({"path":entry.path.to_string_lossy(),"index":index})))
                    .with_child(button(&format!("change-diff-{index}"), "Diff"));
                if entry.unstaged {
                    node = node.with_child(button(&format!("change-stage-{index}"), "Stage"));
                }
                if entry.staged {
                    node = node.with_child(button(&format!("change-unstage-{index}"), "Unstage"));
                }
            }
        }
        node.with_child(
            ViewNode::new("text")
                .with_id("worktree-diff")
                .with_text(
                    &data
                        .diff_rows
                        .iter()
                        .map(|row| row.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .with_children(
                    data.diff_rows
                        .iter()
                        .map(|row| {
                            ViewNode::new(match row.kind {
                                crate::diff_model::RowKind::Added => "diff-added",
                                crate::diff_model::RowKind::Removed => "diff-removed",
                                _ => "diff-row",
                            })
                            .with_state(serde_json::json!({"old_line":row.old_line,"new_line":row.new_line,"highlights":row.highlights.iter().map(|range| vec![range.start,range.end]).collect::<Vec<_>>() }))
                            .with_text(&format!(
                                "{} {} {}",
                                row.old_line.map(|n| n.to_string()).unwrap_or_default(),
                                row.new_line.map(|n| n.to_string()).unwrap_or_default(),
                                row.text
                            ))
                        })
                        .collect(),
                ),
        )
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
        let mut toolbar = ViewNode::new("panel")
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

        if let Some(detail) = &self.detail {
            toolbar = toolbar.with_child(
                ViewNode::new("panel")
                    .with_id("log-branches-scroll")
                    .with_children(
                        detail
                            .branches
                            .iter()
                            .flat_map(|branch| {
                                let mut nodes = vec![
                                    ViewNode::new("button")
                                        .with_id(&format!("log-branch-{branch}"))
                                        .with_text(branch)
                                        .with_interactive(),
                                ];
                                if branch != &detail.current_branch {
                                    nodes.push(
                                        ViewNode::new("button")
                                            .with_id(&format!("log-checkout-{branch}"))
                                            .with_text("Switch")
                                            .with_interactive(),
                                    );
                                    nodes.push(
                                        ViewNode::new("button")
                                            .with_id(&format!("log-delete-{branch}"))
                                            .with_text("Delete")
                                            .with_interactive(),
                                    );
                                }
                                nodes
                            })
                            .collect(),
                    ),
            );
            for remote in &detail.remotes {
                for action in ["fetch", "pull", "push", "reset"] {
                    toolbar = toolbar.with_child(
                        ViewNode::new("button")
                            .with_id(&format!("log-remote-{}-{action}", remote.name))
                            .with_text(&format!("{} {action}", remote.name))
                            .with_interactive(),
                    );
                }
            }
        }
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
                    .with_id(&format!("log-commit-{}", entry.full_hash))
                    .with_interactive()
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
                    node.id.lane_id, node.id.commit_id
                ))
                .with_interactive()
                .with_child(ViewNode::new("text").with_text(node.short_hash()));
            match node.kind {
                CanvasNodeKind::Commit => {
                    if let Some(entry) = node.entry.as_ref() {
                        view = view.with_child(ViewNode::new("text").with_text(&entry.message));
                    }
                }
                CanvasNodeKind::Placeholder => {
                    view = view.with_child(
                        ViewNode::new("text").with_text("Referenced commit not loaded"),
                    );
                }
                CanvasNodeKind::Head => {
                    view = view.with_children(
                        node.head_labels
                            .iter()
                            .map(|label| ViewNode::new("label").with_text(label))
                            .collect(),
                    );
                }
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
        self.context_menu.as_ref()?;
        let mut node = ViewNode::new("menu")
            .with_id("context-menu")
            .with_bounds_from(reg, "context-menu");
        for (id, label) in [
            ("ctx-open-directory", "Open Directory"),
            ("ctx-open-terminal", "Open Terminal Here"),
        ] {
            node = node.with_child(
                ViewNode::new("menu-item")
                    .with_id(id)
                    .with_text(label)
                    .with_interactive()
                    .with_bounds_from(reg, id),
            );
        }

        Some(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_snapshot_exposes_actions_and_hides_other_repository_data() {
        let app = GitMasterApp::new();
        let mut snapshot = app.test_view_tree_snapshot();
        let path = std::path::PathBuf::from("fixture");
        snapshot.detail = Some(RepoDetail {
            path: "fixture".into(),
            current_branch: "main".into(),
            branches: vec![],
            remotes: vec![],
            head_labels: Default::default(),
            file_status: Default::default(),
        });
        snapshot.worktree = Some((
            path,
            Ok(crate::worktree::Snapshot {
                changes: vec![crate::worktree::Change {
                    path: "file.txt".into(),
                    status: "Modified".into(),
                    staged: true,
                    unstaged: true,
                }],
                stashes: vec![crate::worktree::Stash {
                    index: 0,
                    oid: git2::Oid::ZERO_SHA1,
                    message: "Saved edits".into(),
                }],
                diff: "+preview".into(),
                diff_rows: vec![crate::diff_model::DiffRow::label(
                    crate::diff_model::RowKind::Note,
                    "+preview",
                )],
            }),
        ));
        let find = |node: &ViewNode, id: &str| {
            node.children
                .iter()
                .find(|child| child.id.as_deref() == Some(id))
                .cloned()
        };
        let tree = snapshot.build_worktree_node(false, &HashMap::new());
        assert!(find(&tree, "change-stage-0").unwrap().interactive);
        assert!(find(&tree, "change-unstage-0").unwrap().interactive);
        assert_eq!(
            find(&tree, "worktree-diff").unwrap().text.as_deref(),
            Some("+preview")
        );
        let stash_tree = snapshot.build_worktree_node(true, &HashMap::new());
        for action in ["select", "show", "apply", "pop", "drop"] {
            assert!(
                find(
                    &stash_tree,
                    &format!("stash-{action}-{}", git2::Oid::ZERO_SHA1)
                )
                .unwrap()
                .interactive
            );
        }
        snapshot.busy = true;
        let tree = snapshot.build_worktree_node(false, &HashMap::new());
        assert!(!find(&tree, "change-stage-0").unwrap().interactive);
        snapshot.detail.as_mut().unwrap().path = "other-repo".into();
        assert!(
            find(
                &snapshot.build_worktree_node(false, &HashMap::new()),
                "worktree-diff"
            )
            .is_none()
        );
    }

    #[test]
    fn snapshot_builds_a_view_tree_without_accessing_live_ui_state() {
        let app = GitMasterApp::new();

        let tree = app.test_view_tree_snapshot().build();

        assert_eq!(tree.node_type, "window");
        assert_eq!(tree.id.as_deref(), Some("root"));
        assert!(!tree.children.is_empty());
    }
}
