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
        relative_path: PathBuf,
    },
}

#[derive(Clone)]
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
    pub scan_task: Option<Task<()>>,
    pub detail_task: Option<Task<()>>,
    pub context_menu_task: Option<Task<()>>,
    pub push_preflight_task: Option<Task<()>>,
    pub operation_task: Option<Task<()>>,
    #[cfg(feature = "test-rpc")]
    pub test_view_tree_task: Option<Task<()>>,
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
            scan_task: None,
            detail_task: None,
            context_menu_task: None,
            push_preflight_task: None,
            operation_task: None,
            #[cfg(feature = "test-rpc")]
            test_view_tree_task: None,
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
        self.detail_task = None;
        self.context_menu_task = None;
        self.context_menu = None;
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
        self.detail_task = None;
        self.selected = Some(RepoSelection::Repo(index));
        self.active_tab = DetailTab::Info;
        self.detail = None;
        self.submodule_detail = None;
        self.log_entries.clear();
        self.loading_detail = true;
    }

    pub fn begin_select_submodule(
        &mut self,
        repo_index: usize,
        submodule_index: usize,
        relative_path: PathBuf,
    ) {
        self.detail_task = None;
        self.selected = Some(RepoSelection::Submodule {
            repo_index,
            submodule_index,
            relative_path,
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

    pub fn apply_repo_refresh(
        &mut self,
        index: usize,
        expected_path: &Path,
        refreshed: Option<RepoInfo>,
    ) {
        if self.repos.get(index).map(|repo| repo.path.as_path()) != Some(expected_path) {
            return;
        }
        if let Some(info) = refreshed {
            self.repos[index] = info;
            self.reconcile_submodule_selection(index);
        }
    }

    fn reconcile_submodule_selection(&mut self, refreshed_repo_index: usize) {
        let Some(RepoSelection::Submodule {
            repo_index,
            relative_path,
            ..
        }) = self.selected.as_ref()
        else {
            return;
        };
        if *repo_index != refreshed_repo_index {
            return;
        }
        let Some(new_index) = self.repos.get(*repo_index).and_then(|repo| {
            repo.submodules
                .iter()
                .position(|submodule| submodule.relative_path == *relative_path)
        }) else {
            self.selected = None;
            self.detail = None;
            self.submodule_detail = None;
            self.log_entries.clear();
            self.loading_detail = false;
            return;
        };
        self.selected = Some(RepoSelection::Submodule {
            repo_index: *repo_index,
            submodule_index: new_index,
            relative_path: relative_path.clone(),
        });
    }

    pub fn selected_submodule(&self) -> Option<(usize, usize, PathBuf)> {
        match self.selected.as_ref() {
            Some(RepoSelection::Submodule {
                repo_index,
                submodule_index,
                relative_path,
            }) => self
                .repos
                .get(*repo_index)
                .and_then(|repo| repo.submodules.get(*submodule_index))
                .filter(|submodule| submodule.relative_path == *relative_path)
                .map(|_| (*repo_index, *submodule_index, relative_path.clone())),
            _ => None,
        }
    }

    #[cfg(feature = "test-rpc")]
    pub fn prepare_test_command(
        &mut self,
        command: crate::test_rpc::server::TestCommand,
    ) -> Option<TestDetailRequest> {
        match command {
            crate::test_rpc::server::TestCommand::SelectRepo(index) => {
                let path = self.repos.get(index)?.path.clone();
                let selection = RepoSelection::Repo(index);
                self.begin_select(index);
                Some(TestDetailRequest {
                    selection,
                    path,
                    submodule_detail: None,
                    is_initialized: true,
                })
            }
            crate::test_rpc::server::TestCommand::ToggleRepo(index) => {
                self.toggle_repo_expanded(index);
                None
            }
            crate::test_rpc::server::TestCommand::SelectSubmodule {
                repo_index,
                submodule_index,
            } => {
                let submodule = self
                    .repos
                    .get(repo_index)?
                    .submodules
                    .get(submodule_index)?;
                let path = submodule.path.clone();
                let relative_path = submodule.relative_path.clone();
                let submodule_detail = Some(SubmoduleDetail {
                    name: submodule.name.clone(),
                    path: submodule.path.display().to_string(),
                    url: submodule.url.clone(),
                    is_initialized: submodule.is_initialized,
                });
                let is_initialized = submodule.is_initialized;
                let selection = RepoSelection::Submodule {
                    repo_index,
                    submodule_index,
                    relative_path: relative_path.clone(),
                };
                self.begin_select_submodule(repo_index, submodule_index, relative_path);
                Some(TestDetailRequest {
                    selection,
                    path,
                    submodule_detail,
                    is_initialized,
                })
            }
            crate::test_rpc::server::TestCommand::SetTab(tab) => {
                match tab.as_str() {
                    "info" => self.set_tab(DetailTab::Info),
                    "log" => self.set_tab(DetailTab::GitLog),
                    _ => {}
                }
                None
            }
        }
    }

    #[cfg(feature = "test-rpc")]
    pub fn apply_test_detail(&mut self, result: TestDetailResult) {
        self.apply_detail(
            result.selection,
            result.detail,
            result.submodule_detail,
            result.log_entries,
        );
    }

    #[cfg(feature = "test-rpc")]
    pub fn schedule_test_view_tree_publish(&mut self, cx: &mut Context<'_, Self>) {
        let snapshot = self.test_view_tree_snapshot();
        let provider = self.tree_provider.clone();
        self.test_view_tree_task = Some(cx.background_executor().spawn(async move {
            let tree = snapshot.build();
            if let Ok(mut guard) = provider.lock() {
                *guard = Some(tree);
            }
        }));
    }
}

#[cfg(feature = "test-rpc")]
pub struct TestDetailRequest {
    selection: RepoSelection,
    path: PathBuf,
    submodule_detail: Option<SubmoduleDetail>,
    is_initialized: bool,
}

#[cfg(feature = "test-rpc")]
pub struct TestDetailResult {
    selection: RepoSelection,
    detail: Option<RepoDetail>,
    submodule_detail: Option<SubmoduleDetail>,
    log_entries: Vec<LogEntry>,
}

#[cfg(feature = "test-rpc")]
impl TestDetailRequest {
    pub fn load(self) -> TestDetailResult {
        let (detail, log_entries) = if self.is_initialized {
            (
                crate::git_ops::get_repo_detail(&self.path),
                crate::git_ops::get_commit_log(&self.path, 200),
            )
        } else {
            (None, Vec::new())
        };
        TestDetailResult {
            selection: self.selection,
            detail,
            submodule_detail: self.submodule_detail,
            log_entries,
        }
    }
}

impl Render for GitMasterApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        #[cfg(feature = "test-rpc")]
        {
            cx.on_next_frame(window, |this, _window, cx| {
                this.schedule_test_view_tree_publish(cx);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SubmoduleInfo;

    fn repo(name: &str, path: &str, submodules: Vec<SubmoduleInfo>) -> RepoInfo {
        RepoInfo {
            name: name.to_string(),
            path: PathBuf::from(path),
            is_dirty: false,
            ahead: 0,
            behind: 0,
            current_branch: "main".to_string(),
            submodules,
        }
    }

    fn submodule(name: &str, path: &str, relative_path: &str) -> SubmoduleInfo {
        SubmoduleInfo {
            name: name.to_string(),
            path: PathBuf::from(path),
            relative_path: PathBuf::from(relative_path),
            url: None,
            is_initialized: true,
            is_dirty: false,
            ahead: 0,
            behind: 0,
            current_branch: "main".to_string(),
        }
    }

    #[::core::prelude::v1::test]
    fn repo_refresh_ignores_results_for_a_replaced_repo() {
        let mut app = GitMasterApp::new();
        app.repos = vec![repo("current", "/repos/current", Vec::new())];

        app.apply_repo_refresh(
            0,
            Path::new("/repos/previous"),
            Some(repo("stale", "/repos/previous", Vec::new())),
        );

        assert_eq!(app.repos[0].name, "current");
        assert_eq!(app.repos[0].path, PathBuf::from("/repos/current"));
    }

    #[::core::prelude::v1::test]
    fn repo_refresh_reconciles_submodule_selection_by_relative_path() {
        let mut app = GitMasterApp::new();
        app.repos = vec![repo(
            "repo",
            "/repos/repo",
            vec![
                submodule("first", "/repos/repo/first", "first"),
                submodule("target", "/repos/repo/target", "target"),
            ],
        )];
        app.selected = Some(RepoSelection::Submodule {
            repo_index: 0,
            submodule_index: 1,
            relative_path: PathBuf::from("target"),
        });

        app.apply_repo_refresh(
            0,
            Path::new("/repos/repo"),
            Some(repo(
                "repo",
                "/repos/repo",
                vec![
                    submodule("target", "/repos/repo/target", "target"),
                    submodule("first", "/repos/repo/first", "first"),
                ],
            )),
        );

        assert_eq!(
            app.selected,
            Some(RepoSelection::Submodule {
                repo_index: 0,
                submodule_index: 0,
                relative_path: PathBuf::from("target"),
            })
        );
    }

    #[cfg(feature = "test-rpc")]
    #[::core::prelude::v1::test]
    fn test_rpc_selection_prepares_background_detail_work() {
        let mut app = GitMasterApp::new();
        app.repos = vec![repo("repo", "/repos/repo", Vec::new())];

        let request = app.prepare_test_command(crate::test_rpc::server::TestCommand::SelectRepo(0));

        assert!(request.is_some());
        assert!(app.loading_detail);
        assert_eq!(app.selected, Some(RepoSelection::Repo(0)));
    }
}
