use std::path::{Path, PathBuf};

use gpui::*;

use crate::models::{LogEntry, RepoDetail, RepoInfo};
use crate::ui::theme;

#[derive(Clone, Copy, PartialEq)]
pub enum DetailTab {
    Info,
    GitLog,
}

pub struct ContextMenu {
    pub repo_index: usize,
    pub position: Point<Pixels>,
    pub branches: Vec<String>,
    pub show_branches: bool,
}

pub struct GitMasterApp {
    pub parent_dir: Option<PathBuf>,
    pub repos: Vec<RepoInfo>,
    pub selected_index: Option<usize>,
    pub active_tab: DetailTab,
    pub detail: Option<RepoDetail>,
    pub log_entries: Vec<LogEntry>,
    pub scanning: bool,
    pub loading_detail: bool,
    pub context_menu: Option<ContextMenu>,
    pub status_message: Option<String>,
    pub busy: bool,
    #[cfg(feature = "test-rpc")]
    pub bounds_registry: crate::test_rpc::tracked::BoundsRegistry,
    #[cfg(feature = "test-rpc")]
    pub tree_provider: crate::test_rpc::server::ViewTreeProvider,
    #[cfg(feature = "test-rpc")]
    pub command_queue: crate::test_rpc::server::CommandQueue,
}

impl GitMasterApp {
    pub fn new() -> Self {
        Self {
            parent_dir: None,
            repos: Vec::new(),
            selected_index: None,
            active_tab: DetailTab::Info,
            detail: None,
            log_entries: Vec::new(),
            scanning: false,
            loading_detail: false,
            context_menu: None,
            status_message: None,
            busy: false,
            #[cfg(feature = "test-rpc")]
            bounds_registry: Default::default(),
            #[cfg(feature = "test-rpc")]
            tree_provider: Default::default(),
            #[cfg(feature = "test-rpc")]
            command_queue: Default::default(),
        }
    }

    /// Mark a directory as the active parent and enter the scanning state.
    /// The actual `scan_repos` work happens off-thread; results land via
    /// [`apply_scan`].
    pub fn begin_scan(&mut self, path: PathBuf) {
        self.parent_dir = Some(path);
        self.repos.clear();
        self.selected_index = None;
        self.detail = None;
        self.log_entries.clear();
        self.scanning = true;
        self.loading_detail = false;
    }

    /// Apply scan results, ignoring stale completions from a directory the
    /// user has since navigated away from.
    pub fn apply_scan(&mut self, path: &Path, repos: Vec<RepoInfo>) {
        if self.parent_dir.as_deref() != Some(path) {
            return;
        }
        self.repos = repos;
        self.scanning = false;
    }

    /// Mark a repo as selected and enter the loading state. The detail and
    /// commit-log work happens off-thread; results land via [`apply_detail`].
    pub fn begin_select(&mut self, index: usize) {
        self.selected_index = Some(index);
        self.active_tab = DetailTab::Info;
        self.detail = None;
        self.log_entries.clear();
        self.loading_detail = true;
    }

    /// Apply detail results, ignoring stale completions for a repo other than
    /// the one currently selected.
    pub fn apply_detail(
        &mut self,
        index: usize,
        detail: Option<RepoDetail>,
        log_entries: Vec<LogEntry>,
    ) {
        if self.selected_index != Some(index) {
            return;
        }
        self.detail = detail;
        self.log_entries = log_entries;
        self.loading_detail = false;
    }

    pub fn set_tab(&mut self, tab: DetailTab) {
        self.active_tab = tab;
    }

    pub fn open_context_menu(
        &mut self,
        repo_index: usize,
        position: Point<Pixels>,
        branches: Vec<String>,
    ) {
        self.context_menu = Some(ContextMenu {
            repo_index,
            position,
            branches,
            show_branches: false,
        });
    }

    pub fn close_context_menu(&mut self) {
        self.context_menu = None;
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
    }

    #[cfg(feature = "test-rpc")]
    pub fn track(&self, id: &str, element: impl IntoElement) -> AnyElement {
        crate::test_rpc::tracked::tracked(id, element, &self.bounds_registry).into_any_element()
    }

    #[cfg(not(feature = "test-rpc"))]
    pub fn track(&self, _id: &str, element: impl IntoElement) -> AnyElement {
        element.into_any_element()
    }

    pub fn refresh_repo(&mut self, index: usize) {
        if let Some(repo) = self.repos.get(index) {
            if let Some(info) = crate::git_ops::build_repo_info(&repo.path) {
                self.repos[index] = info;
            }
        }
    }
}

impl Render for GitMasterApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        #[cfg(feature = "test-rpc")]
        {
            cx.on_next_frame(window, |this, _window, cx| {
                let cmds: Vec<_> = this
                    .command_queue
                    .lock()
                    .ok()
                    .map(|mut q| q.drain(..).collect())
                    .unwrap_or_default();
                let mut changed = false;
                for cmd in cmds {
                    match cmd {
                        crate::test_rpc::server::TestCommand::SelectRepo(i) => {
                            if let Some(repo) = this.repos.get(i) {
                                let path = repo.path.clone();
                                this.begin_select(i);
                                let detail = crate::git_ops::get_repo_detail(&path);
                                let log = crate::git_ops::get_commit_log(&path, 200);
                                this.apply_detail(i, detail, log);
                                changed = true;
                            }
                        }
                        crate::test_rpc::server::TestCommand::SetTab(ref tab) => {
                            match tab.as_str() {
                                "info" => this.set_tab(DetailTab::Info),
                                "log" => this.set_tab(DetailTab::GitLog),
                                _ => {}
                            }
                            changed = true;
                        }
                    }
                }
                if changed {
                    cx.notify();
                }
                let tree = this.build_view_tree();
                if let Ok(mut guard) = this.tree_provider.lock() {
                    *guard = Some(tree);
                }
            });
        }

        let top_bar = self.render_top_bar(window, cx);
        let repo_list = self.render_repo_list(window, cx);
        let detail_panel = self.render_detail_panel(window, cx);
        let context_menu = self.render_context_menu(window, cx);

        let main_content = div()
            .flex()
            .flex_row()
            .flex_grow()
            .child(self.track("repo-list-panel", repo_list))
            .children(detail_panel.map(|p| self.track("detail-panel", p)));
        let main_content = self.track("main-content", main_content);

        let root = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(theme::BG_BASE))
            .text_color(rgb(theme::TEXT_PRIMARY))
            .child(self.track("top-bar", top_bar))
            .child(main_content)
            .children(context_menu.map(|m| self.track("context-menu", m)));

        self.track("root", root)
    }
}
