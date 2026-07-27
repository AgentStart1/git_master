use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use gpui::*;

use crate::models::{LogEntry, RepoDetail, RepoInfo, SubmoduleDetail};
use crate::ui::theme;

#[derive(Clone, Copy, PartialEq)]
pub enum DetailTab {
    Info,
    GitLog,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepoSelection {
    Repo(usize),
    Submodule {
        repo_index: usize,
        submodule_index: usize,
    },
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
    pub selected: Option<RepoSelection>,
    pub expanded_repos: BTreeSet<usize>,
    pub active_tab: DetailTab,
    pub detail: Option<RepoDetail>,
    pub submodule_detail: Option<SubmoduleDetail>,
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
            selected: None,
            expanded_repos: BTreeSet::new(),
            active_tab: DetailTab::Info,
            detail: None,
            submodule_detail: None,
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
        self.selected = None;
        self.expanded_repos.clear();
        self.detail = None;
        self.submodule_detail = None;
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
        self.selected = Some(RepoSelection::Repo(index));
        self.active_tab = DetailTab::Info;
        self.detail = None;
        self.submodule_detail = None;
        self.log_entries.clear();
        self.loading_detail = true;
    }

    pub fn begin_select_submodule(&mut self, repo_index: usize, submodule_index: usize) {
        self.selected = Some(RepoSelection::Submodule {
            repo_index,
            submodule_index,
        });
        self.active_tab = DetailTab::Info;
        self.detail = None;
        self.submodule_detail = None;
        self.log_entries.clear();
        self.loading_detail = true;
    }

    /// Apply detail results, ignoring stale completions for a repo other than
    /// the one currently selected.
    pub fn apply_detail(
        &mut self,
        selection: RepoSelection,
        detail: Option<RepoDetail>,
        submodule_detail: Option<SubmoduleDetail>,
        log_entries: Vec<LogEntry>,
    ) {
        if self.selected.as_ref() != Some(&selection) {
            return;
        }
        self.detail = detail;
        self.submodule_detail = submodule_detail;
        self.log_entries = log_entries;
        self.loading_detail = false;
    }

    pub fn set_tab(&mut self, tab: DetailTab) {
        self.active_tab = tab;
    }

    pub fn toggle_repo_expanded(&mut self, index: usize) {
        if !self.expanded_repos.insert(index) {
            self.expanded_repos.remove(&index);
        }
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

    pub fn selected_submodule(&self) -> Option<(usize, usize)> {
        match self.selected {
            Some(RepoSelection::Submodule {
                repo_index,
                submodule_index,
            }) => Some((repo_index, submodule_index)),
            _ => None,
        }
    }

    #[cfg(feature = "test-rpc")]
    pub fn process_test_commands(&mut self) -> bool {
        let cmds: Vec<_> = self
            .command_queue
            .lock()
            .ok()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default();
        let mut changed = false;
        for cmd in cmds {
            match cmd {
                crate::test_rpc::server::TestCommand::SelectRepo(i) => {
                    if let Some(repo) = self.repos.get(i) {
                        let path = repo.path.clone();
                        self.begin_select(i);
                        let detail = crate::git_ops::get_repo_detail(&path);
                        let log = crate::git_ops::get_commit_log(&path, 200);
                        self.apply_detail(RepoSelection::Repo(i), detail, None, log);
                        changed = true;
                    }
                }
                crate::test_rpc::server::TestCommand::ToggleRepo(i) => {
                    self.toggle_repo_expanded(i);
                    changed = true;
                }
                crate::test_rpc::server::TestCommand::SelectSubmodule {
                    repo_index,
                    submodule_index,
                } => {
                    if let Some((path, submodule_detail, is_initialized)) = self
                        .repos
                        .get(repo_index)
                        .and_then(|repo| repo.submodules.get(submodule_index))
                        .map(|submodule| {
                            (
                                submodule.path.clone(),
                                Some(SubmoduleDetail {
                                    name: submodule.name.clone(),
                                    path: submodule.path.display().to_string(),
                                    url: submodule.url.clone(),
                                    is_initialized: submodule.is_initialized,
                                }),
                                submodule.is_initialized,
                            )
                        })
                    {
                        self.begin_select_submodule(repo_index, submodule_index);
                        let detail = is_initialized
                            .then(|| crate::git_ops::get_repo_detail(&path))
                            .flatten();
                        let log = if is_initialized {
                            crate::git_ops::get_commit_log(&path, 200)
                        } else {
                            Vec::new()
                        };
                        self.apply_detail(
                            RepoSelection::Submodule {
                                repo_index,
                                submodule_index,
                            },
                            detail,
                            submodule_detail,
                            log,
                        );
                        changed = true;
                    }
                }
                crate::test_rpc::server::TestCommand::SetTab(ref tab) => {
                    match tab.as_str() {
                        "info" => self.set_tab(DetailTab::Info),
                        "log" => self.set_tab(DetailTab::GitLog),
                        _ => {}
                    }
                    changed = true;
                }
            }
        }
        changed
    }

    #[cfg(feature = "test-rpc")]
    pub fn publish_test_view_tree(&self) {
        let tree = self.build_view_tree();
        if let Ok(mut guard) = self.tree_provider.lock() {
            *guard = Some(tree);
        }
    }
}

impl Render for GitMasterApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        #[cfg(feature = "test-rpc")]
        {
            cx.on_next_frame(window, |this, _window, cx| {
                let changed = this.process_test_commands();
                if changed {
                    cx.notify();
                }
                this.publish_test_view_tree();
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
