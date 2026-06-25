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
    fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
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

    fn with_bounds(mut self, bounds: Rect) -> Self {
        self.bounds = Some(bounds);
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

const WINDOW_W: f32 = 1200.0;
const WINDOW_H: f32 = 800.0;
const TOP_BAR_H: f32 = 44.0;
const REPO_LIST_W: f32 = 280.0;
const REPO_ITEM_H: f32 = 40.0;
const TAB_BAR_H: f32 = 36.0;
const LOG_ENTRY_H: f32 = 40.0;

impl GitMasterApp {
    pub fn build_view_tree(&self) -> ViewNode {
        let top_bar = self.build_top_bar_node();
        let repo_list = self.build_repo_list_node();
        let detail_panel = self.build_detail_panel_node();

        let mut main_content = ViewNode::new("panel")
            .with_id("main-content")
            .with_bounds(Rect::new(0.0, TOP_BAR_H, WINDOW_W, WINDOW_H - TOP_BAR_H))
            .with_child(repo_list);

        if let Some(panel) = detail_panel {
            main_content = main_content.with_child(panel);
        }

        let mut root = ViewNode::new("window")
            .with_id("root")
            .with_bounds(Rect::new(0.0, 0.0, WINDOW_W, WINDOW_H))
            .with_child(top_bar)
            .with_child(main_content);

        if let Some(menu) = self.build_context_menu_node() {
            root = root.with_child(menu);
        }

        root
    }

    fn build_top_bar_node(&self) -> ViewNode {
        let dir_label = self
            .parent_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "No directory selected".into());

        let mut bar = ViewNode::new("panel")
            .with_id("top-bar")
            .with_bounds(Rect::new(0.0, 0.0, WINDOW_W, TOP_BAR_H))
            .with_child(ViewNode::new("text").with_text(&dir_label));

        if let Some(msg) = &self.status_message {
            bar = bar.with_child(ViewNode::new("text").with_id("status-message").with_text(msg));
        }

        bar = bar.with_child(
            ViewNode::new("button")
                .with_id("change-dir-btn")
                .with_text("Open Directory")
                .with_interactive()
                .with_bounds(Rect::new(WINDOW_W - 130.0, 6.0, 118.0, 32.0)),
        );

        bar
    }

    fn build_repo_list_node(&self) -> ViewNode {
        let mut list = ViewNode::new("panel")
            .with_id("repo-list")
            .with_bounds(Rect::new(0.0, TOP_BAR_H, REPO_LIST_W, WINDOW_H - TOP_BAR_H));

        if self.scanning {
            list = list.with_child(ViewNode::new("text").with_text("Scanning…"));
        }

        let items: Vec<ViewNode> = self
            .repos
            .iter()
            .enumerate()
            .map(|(i, repo)| {
                let y = TOP_BAR_H + (i as f32) * REPO_ITEM_H;

                let dirty_text = if repo.is_dirty { "●" } else { "✓" };
                let ab = if repo.ahead > 0 || repo.behind > 0 {
                    Some(format!("↑{} ↓{}", repo.ahead, repo.behind))
                } else {
                    None
                };

                let mut item = ViewNode::new("list-item")
                    .with_id(&format!("repo-{i}"))
                    .with_interactive()
                    .with_bounds(Rect::new(0.0, y, REPO_LIST_W, REPO_ITEM_H))
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

    fn build_detail_panel_node(&self) -> Option<ViewNode> {
        self.selected_index?;

        let panel_x = REPO_LIST_W;
        let panel_w = WINDOW_W - REPO_LIST_W;

        let info_tab = ViewNode::new("tab")
            .with_id("tab-info")
            .with_text("Info")
            .with_interactive()
            .with_bounds(Rect::new(panel_x, TOP_BAR_H, 60.0, TAB_BAR_H));

        let log_tab = ViewNode::new("tab")
            .with_id("tab-log")
            .with_text("Git Log")
            .with_interactive()
            .with_bounds(Rect::new(panel_x + 60.0, TOP_BAR_H, 80.0, TAB_BAR_H));

        let tab_bar = ViewNode::new("panel")
            .with_id("tab-bar")
            .with_bounds(Rect::new(panel_x, TOP_BAR_H, panel_w, TAB_BAR_H))
            .with_child(info_tab)
            .with_child(log_tab);

        let content_y = TOP_BAR_H + TAB_BAR_H;
        let content_h = WINDOW_H - TOP_BAR_H - TAB_BAR_H;

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
            .with_bounds(Rect::new(panel_x, TOP_BAR_H, panel_w, WINDOW_H - TOP_BAR_H))
            .with_child(tab_bar)
            .with_child(
                ViewNode::new("panel")
                    .with_id("detail-content")
                    .with_bounds(Rect::new(panel_x, content_y, panel_w, content_h))
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
            .with_child(
                ViewNode::new("label")
                    .with_text(&format!("Path: {}", detail.path)),
            )
            .with_child(
                ViewNode::new("label")
                    .with_text(&format!("Branch: {}", detail.current_branch)),
            )
            .with_child(
                ViewNode::new("label").with_text(&format!(
                    "Remote: {}",
                    detail.remote_url.as_deref().unwrap_or("(none)")
                )),
            )
            .with_child(
                ViewNode::new("label")
                    .with_text(&format!("File Status: {status_text}")),
            )
    }

    fn build_log_node(&self) -> ViewNode {
        let entries: Vec<ViewNode> = self
            .log_entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let y = TOP_BAR_H + TAB_BAR_H + (i as f32) * LOG_ENTRY_H;
                ViewNode::new("list-item")
                    .with_bounds(Rect::new(
                        REPO_LIST_W,
                        y,
                        WINDOW_W - REPO_LIST_W,
                        LOG_ENTRY_H,
                    ))
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

    fn build_context_menu_node(&self) -> Option<ViewNode> {
        let menu = self.context_menu.as_ref()?;
        let pos_x: f32 = menu.position.x.into();
        let pos_y: f32 = menu.position.y.into();
        let current_branch = self
            .repos
            .get(menu.repo_index)
            .map(|r| r.current_branch.as_str())
            .unwrap_or_default();

        let mut node = ViewNode::new("menu")
            .with_id("context-menu")
            .with_bounds(Rect::new(pos_x, pos_y, 200.0, 0.0))
            .with_child(
                ViewNode::new("menu-item")
                    .with_id("ctx-switch-branch")
                    .with_text(if menu.show_branches {
                        "▾ Switch Branch"
                    } else {
                        "▸ Switch Branch"
                    })
                    .with_interactive(),
            );

        if menu.show_branches {
            let branch_items: Vec<ViewNode> = menu
                .branches
                .iter()
                .filter(|b| b.as_str() != current_branch)
                .map(|b| {
                    ViewNode::new("menu-item")
                        .with_id(&format!("ctx-branch-{b}"))
                        .with_text(b)
                        .with_interactive()
                })
                .collect();
            node = node.with_children(branch_items);
        }

        node = node
            .with_child(
                ViewNode::new("menu-item")
                    .with_id("ctx-pull-rebase")
                    .with_text("Pull --rebase")
                    .with_interactive(),
            )
            .with_child(
                ViewNode::new("menu-item")
                    .with_id("ctx-push")
                    .with_text("Push")
                    .with_interactive(),
            );

        Some(node)
    }
}
