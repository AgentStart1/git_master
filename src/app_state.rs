use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use gpui::*;

use crate::models::{LogEntry, RepoDetail, RepoInfo, SubmoduleDetail};
use crate::ui::commit_canvas::{
    CanvasInteraction, CommitCanvasLayout, CommitCanvasState, LogViewMode,
};
use crate::ui::theme;

#[derive(Clone, Copy, PartialEq)]
pub enum DetailTab {
    Info,
    GitLog,
    GitChange,
    GitStash,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetailRequest {
    pub path: PathBuf,
    pub revision: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskOutcome {
    #[default]
    Success,
    Partial,
    Failure,
    Warning,
    Skipped,
}

#[derive(Clone)]
pub struct ContextMenu {
    pub target_path: Option<PathBuf>,
    pub repo_index: usize,
    pub position: Point<Pixels>,
}

pub struct GitMasterApp {
    pub(crate) canvas_route_cache: std::cell::RefCell<crate::ui::canvas_routes::RouteCache>,
    pub popup_focus: HashMap<crate::ui::popup::PopupKind, FocusHandle>,
    pub blocked_focus: Option<FocusHandle>,
    pub initialization: HashMap<PathBuf, crate::ui::state_views::Initialization>,
    pub blocked_action: Option<crate::ui::blocked_action::BlockedAction>,
    pub conflict_collapsed: BTreeSet<String>,
    pub batch_expanded: BTreeSet<usize>,
    pub repo_search: Option<Entity<crate::ui::text_input::TextInput>>,
    pub branch_search: Option<Entity<crate::ui::text_input::TextInput>>,
    pub repo_filter: crate::ui::workspace_controls::RepoFilter,
    pub workspace_menu_open: bool,
    pub remote_menu: Option<String>,
    pub remote_picker_open: bool,
    pub overview_reference: Option<String>,
    pub overview_ref_picker: bool,
    pub last_fetch: HashMap<(PathBuf, String), String>,
    pub overview_remote: Option<String>,
    pub recent_workspaces: Vec<PathBuf>,
    pub narrow_window: bool,
    pub canvas_reveal_selection: bool,
    pub canvas_viewport_size: Option<Size<Pixels>>,
    pub branch_menu_focus: Option<FocusHandle>,
    pub branch_trigger_focus: HashMap<crate::ui::branch_filter::BranchPicker, FocusHandle>,
    pub branch_keyboard_index: Option<usize>,
    pub narrow_details_open: bool,
    pub batch_result: Option<crate::batch_ops::BatchResult>,
    pub branch_picker: Option<crate::ui::branch_filter::BranchPicker>,

    pub log_window: Option<WindowHandle<crate::ui::log_viewer::LogsWindow>>,
    pub log_window_generation: u64,
    #[cfg(feature = "test-rpc")]
    pub logs_tree_provider: crate::test_rpc::server::ViewTreeProvider,
    #[cfg(feature = "test-rpc")]
    pub test_prompt: crate::test_rpc::prompt::PromptState,
    pub worktree: Option<(PathBuf, Result<crate::worktree::Snapshot, String>)>,
    pub worktree_task: Option<Task<()>>,
    pub worktree_loading: bool,
    pub preview_section: crate::ui::worktree_views::PreviewSection,
    pub worktree_revision: u64,
    pub navigation_revision: u64,
    pub detail_revision: u64,
    pub worktree_preview: crate::preview_state::PreviewState,
    pub selected_commit: Option<(PathBuf, String)>,
    pub commit_expanded_file: Option<PathBuf>,
    pub commit_message_source: bool,
    pub commit_file_preview: Option<Result<Vec<crate::diff_model::DiffRow>, String>>,
    pub commit_file_task: Option<Task<()>>,
    pub commit_presentation: Option<crate::commit_details::Presentation>,
    pub commit_text: Option<Result<String, String>>,
    pub commit_task: Option<Task<()>>,
    pub commit_press: Option<crate::ui::commit_details::CommitPress>,
    pub parent_dir: Option<PathBuf>,
    pub repos: Vec<RepoInfo>,
    pub selected: Option<RepoSelection>,
    pub collapsed_repo_groups: BTreeSet<bool>,
    pub expanded_repos: BTreeSet<usize>,
    pub repo_scroll: ScrollHandle,
    pub repo_scroll_drag: Option<f32>,
    pub active_tab: DetailTab,
    pub detail: Option<RepoDetail>,
    pub submodule_detail: Option<SubmoduleDetail>,
    pub log_entries: Vec<LogEntry>,
    pub log_view_mode: LogViewMode,
    pub log_visible_branches: BTreeSet<String>,
    pub history_host: crate::history_host::HistoryHost,
    pub history_task: Option<Task<()>>,
    pub commit_canvas_layout: Option<CommitCanvasLayout>,
    pub commit_canvas_states: HashMap<PathBuf, CommitCanvasState>,
    pub commit_canvas_interaction: Option<CanvasInteraction>,
    pub scanning: bool,
    pub loading_detail: bool,
    pub context_menu: Option<ContextMenu>,
    pub status_outcome: TaskOutcome,
    pub status_message: Option<String>,
    pub status_expanded: bool,
    #[cfg(feature = "test-rpc")]
    pub status_revision: u64,
    pub busy: bool,
    pub scan_task: Option<Task<()>>,
    pub workspace_poll_task: Option<Task<()>>,
    pub directory_prompt_task: Option<Task<()>>,
    pub detail_task: Option<Task<()>>,
    pub remote_action_prompt_task: Option<Task<()>>,
    pub operation_task: Option<Task<()>>,
    #[cfg(feature = "test-rpc")]
    pub test_view_tree_task: Option<Task<()>>,
    #[cfg(feature = "test-rpc")]
    pub test_view_revision: u64,
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
            canvas_route_cache: Default::default(),
            popup_focus: HashMap::new(),
            blocked_focus: None,
            initialization: HashMap::new(),
            blocked_action: None,
            conflict_collapsed: BTreeSet::new(),
            batch_expanded: BTreeSet::new(),
            repo_search: None,
            branch_search: None,
            repo_filter: Default::default(),
            workspace_menu_open: false,
            remote_menu: None,
            remote_picker_open: false,
            overview_reference: None,
            overview_ref_picker: false,
            last_fetch: HashMap::new(),
            overview_remote: None,
            recent_workspaces: Vec::new(),
            narrow_window: false,
            canvas_reveal_selection: false,
            canvas_viewport_size: None,
            branch_menu_focus: None,
            branch_trigger_focus: HashMap::new(),
            branch_keyboard_index: None,
            narrow_details_open: false,
            batch_result: None,
            branch_picker: None,
            log_window: None,
            log_window_generation: 0,
            #[cfg(feature = "test-rpc")]
            logs_tree_provider: Default::default(),
            #[cfg(feature = "test-rpc")]
            test_prompt: Default::default(),
            worktree: None,
            worktree_task: None,
            worktree_loading: false,
            preview_section: Default::default(),
            worktree_revision: 0,
            navigation_revision: 0,
            detail_revision: 0,
            worktree_preview: Default::default(),
            selected_commit: None,
            commit_expanded_file: None,
            commit_message_source: false,
            commit_file_preview: None,
            commit_file_task: None,
            commit_presentation: None,
            commit_text: None,
            commit_task: None,
            commit_press: None,
            parent_dir: None,
            repos: Vec::new(),
            selected: None,
            collapsed_repo_groups: BTreeSet::new(),
            expanded_repos: BTreeSet::new(),
            repo_scroll: ScrollHandle::new(),
            repo_scroll_drag: None,
            active_tab: DetailTab::GitLog,
            detail: None,
            submodule_detail: None,
            log_entries: Vec::new(),
            log_view_mode: LogViewMode::List,
            log_visible_branches: BTreeSet::new(),
            history_host: Default::default(),
            history_task: None,
            commit_canvas_layout: None,
            commit_canvas_states: HashMap::new(),
            commit_canvas_interaction: None,
            scanning: false,
            loading_detail: false,
            context_menu: None,
            status_outcome: TaskOutcome::Success,
            status_message: None,
            status_expanded: false,
            #[cfg(feature = "test-rpc")]
            status_revision: 0,
            busy: false,
            scan_task: None,
            workspace_poll_task: None,
            directory_prompt_task: None,
            detail_task: None,
            remote_action_prompt_task: None,
            operation_task: None,
            #[cfg(feature = "test-rpc")]
            test_view_tree_task: None,
            #[cfg(feature = "test-rpc")]
            test_view_revision: 0,
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
        self.branch_picker = None;
        self.cancel_history_load();
        self.navigation_revision += 1;
        self.detail_revision += 1;
        self.invalidate_worktree();
        self.close_commit_details();
        self.parent_dir = Some(path);
        self.repos.clear();
        self.repo_scroll.set_offset(point(px(0.0), px(0.0)));
        self.repo_scroll_drag = None;
        self.selected = None;
        self.expanded_repos.clear();
        self.detail = None;
        self.submodule_detail = None;
        self.log_entries.clear();
        self.log_visible_branches.clear();
        self.commit_canvas_layout = None;
        self.commit_canvas_interaction = None;
        self.scanning = true;
        self.loading_detail = false;
        self.detail_task = None;
        self.context_menu = None;
    }

    /// Apply scan results, ignoring stale completions from a directory the
    /// user has since navigated away from.
    pub fn apply_scan(&mut self, path: &Path, repos: Vec<RepoInfo>) {
        if self.parent_dir.as_deref() != Some(path) {
            return;
        }
        let target = self
            .selected_remote_action_target()
            .map(|(_, root, target, _, _)| (root, target));
        let expanded_paths = self
            .expanded_repos
            .iter()
            .filter_map(|i| self.repos.get(*i).map(|r| r.path.clone()))
            .collect::<BTreeSet<_>>();
        self.repos = repos;
        self.expanded_repos = self
            .repos
            .iter()
            .enumerate()
            .filter_map(|(i, r)| expanded_paths.contains(&r.path).then_some(i))
            .collect();
        self.close_context_menu();
        if let Some((root, target)) = target {
            self.selected = self
                .repos
                .iter()
                .position(|r| r.path == root)
                .and_then(|index| {
                    if root == target {
                        Some(RepoSelection::Repo(index))
                    } else {
                        self.repos[index]
                            .submodules
                            .iter()
                            .enumerate()
                            .find(|(_, s)| s.path == target)
                            .map(|(submodule_index, s)| RepoSelection::Submodule {
                                repo_index: index,
                                submodule_index,
                                relative_path: s.relative_path.clone(),
                            })
                    }
                });
            if self.selected.is_none() {
                self.detail_revision += 1;
                self.navigation_revision += 1;
                self.detail_task = None;
                self.loading_detail = false;
                self.cancel_history_load();
                self.log_entries.clear();
                self.commit_canvas_layout = None;
                self.detail = None;
                self.submodule_detail = None;
                self.close_commit_details();
                self.invalidate_worktree();
            }
        }
        self.scanning = false;
    }

    /// Mark a repo as selected and enter the loading state. The detail and
    /// commit-log work happens off-thread; results land via [`apply_detail`].
    pub fn begin_select(&mut self, index: usize) {
        self.remote_menu = None;
        self.remote_picker_open = false;
        self.overview_reference = None;
        self.overview_ref_picker = false;
        self.branch_picker = None;
        self.cancel_history_load();
        self.navigation_revision += 1;
        self.detail_revision += 1;
        self.invalidate_worktree();
        self.close_commit_details();
        self.detail_task = None;
        self.selected = Some(RepoSelection::Repo(index));
        self.active_tab = DetailTab::GitLog;
        self.detail = None;
        self.submodule_detail = None;
        self.log_entries.clear();
        self.log_visible_branches.clear();
        self.commit_canvas_layout = None;
        self.commit_canvas_interaction = None;
        self.loading_detail = true;
    }

    pub fn begin_select_submodule(
        &mut self,
        repo_index: usize,
        submodule_index: usize,
        relative_path: PathBuf,
    ) {
        self.branch_picker = None;
        self.cancel_history_load();
        self.navigation_revision += 1;
        self.detail_revision += 1;
        self.invalidate_worktree();
        self.close_commit_details();
        self.detail_task = None;
        self.selected = Some(RepoSelection::Submodule {
            repo_index,
            submodule_index,
            relative_path,
        });
        self.active_tab = DetailTab::GitLog;
        self.detail = None;
        self.submodule_detail = None;
        self.log_entries.clear();
        self.log_visible_branches.clear();
        self.commit_canvas_layout = None;
        self.commit_canvas_interaction = None;
        self.loading_detail = true;
    }

    /// Apply detail results, ignoring stale completions for a repo other than
    /// the one currently selected.
    pub fn apply_detail(
        &mut self,
        request: DetailRequest,
        detail: Option<RepoDetail>,
        submodule_detail: Option<SubmoduleDetail>,
        log_entries: Vec<LogEntry>,
        commit_canvas_layout: Option<CommitCanvasLayout>,
    ) {
        if !self.owns_detail_request(&request) {
            return;
        }
        self.branch_picker = None;
        self.cancel_history_load();
        self.detail = detail;
        self.submodule_detail = submodule_detail;
        self.log_entries = commit_canvas_layout
            .as_ref()
            .map(|layout| layout.list.entries.clone())
            .unwrap_or(log_entries);
        if let Some(layout) = commit_canvas_layout.as_ref() {
            self.commit_canvas_states
                .entry(layout.repository_path.clone())
                .or_default()
                .ensure_layout(layout);
        }
        self.commit_canvas_layout = commit_canvas_layout;
        self.commit_canvas_interaction = None;
        self.loading_detail = false;
    }

    pub fn begin_detail_request(&mut self, path: PathBuf) -> DetailRequest {
        self.detail_revision += 1;
        DetailRequest {
            path,
            revision: self.detail_revision,
        }
    }

    pub fn owns_detail_request(&self, request: &DetailRequest) -> bool {
        request.revision == self.detail_revision
            && self.selected_path().as_ref() == Some(&request.path)
    }

    pub fn selected_path(&self) -> Option<PathBuf> {
        self.selected_remote_action_target()
            .map(|(_, _, path, _, _)| path)
    }

    pub fn set_tab(&mut self, tab: DetailTab) {
        if self.active_tab != tab {
            self.worktree_preview = Default::default();
        }
        self.active_tab = tab;
    }

    pub fn invalidate_worktree(&mut self) {
        self.worktree_preview = Default::default();
        self.worktree_revision += 1;
        self.worktree_task = None;
        self.worktree_loading = false;
        self.worktree = None;
    }

    pub fn worktree_selection(&self) -> Option<String> {
        self.worktree_preview
            .request()
            .map(|request| match &request.target {
                crate::preview_state::PreviewTarget::File(path) => path.display().to_string(),
                crate::preview_state::PreviewTarget::Stash(oid) => oid.to_string(),
            })
    }

    pub fn owns_worktree_request(&self, revision: u64) -> bool {
        self.worktree_revision == revision
    }

    pub fn set_log_view_mode(&mut self, mode: LogViewMode) {
        self.log_view_mode = mode;
        self.commit_canvas_interaction = None;
    }

    pub fn toggle_repo_expanded(&mut self, index: usize) {
        if !self.expanded_repos.insert(index) {
            self.expanded_repos.remove(&index);
        }
    }

    pub fn open_context_menu(&mut self, repo_index: usize, position: Point<Pixels>) {
        self.context_menu = Some(ContextMenu {
            target_path: None,
            repo_index,
            position,
        });
    }

    pub fn close_context_menu(&mut self) {
        self.context_menu = None;
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_outcome = TaskOutcome::Success;
        self.batch_result = None;
        #[cfg(feature = "test-rpc")]
        {
            self.status_revision += 1;
        }
        self.status_message = Some(msg.into());
        self.status_expanded = false;
    }

    pub fn set_failure(&mut self, message: impl Into<String>) {
        self.set_status(message);
        self.status_outcome = TaskOutcome::Failure;
    }
    pub fn set_warning(&mut self, message: impl Into<String>) {
        self.set_status(message);
        self.status_outcome = TaskOutcome::Warning;
    }
    pub fn set_skipped(&mut self, message: impl Into<String>) {
        self.set_status(message);
        self.status_outcome = TaskOutcome::Skipped;
    }

    #[cfg(feature = "test-rpc")]
    pub fn track(&self, id: &str, element: impl IntoElement) -> AnyElement {
        crate::test_rpc::tracked::tracked(id, element, &self.bounds_registry).into_any_element()
    }

    #[cfg(not(feature = "test-rpc"))]
    pub fn track(&self, _id: &str, element: impl IntoElement) -> AnyElement {
        element.into_any_element()
    }

    pub fn apply_repo_refresh(&mut self, expected_path: &Path, refreshed: Option<RepoInfo>) {
        let Some(index) = self
            .repos
            .iter()
            .position(|repo| repo.path == expected_path)
        else {
            return;
        };
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
            self.branch_picker = None;
            self.cancel_history_load();
            self.navigation_revision += 1;
            self.detail_revision += 1;
            self.invalidate_worktree();
            self.close_commit_details();
            self.detail_task = None;
            self.selected = None;
            self.detail = None;
            self.submodule_detail = None;
            self.log_entries.clear();
            self.commit_canvas_layout = None;
            self.commit_canvas_interaction = None;
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
                let graph_repo = self.repos.get(index)?.clone();
                let path = graph_repo.path.clone();

                self.begin_select(index);
                let request = self.begin_detail_request(path.clone());
                Some(TestDetailRequest {
                    request,
                    path,
                    submodule_detail: None,
                    is_initialized: true,
                    graph_path: Some(graph_repo.path),
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
                self.begin_select_submodule(repo_index, submodule_index, relative_path);
                let request = self.begin_detail_request(path.clone());
                Some(TestDetailRequest {
                    request,
                    path,
                    submodule_detail,
                    is_initialized,
                    graph_path: is_initialized
                        .then(|| {
                            self.repos
                                .get(repo_index)
                                .and_then(|repo| repo.submodules.get(submodule_index))
                                .map(|submodule| submodule.path.clone())
                        })
                        .flatten(),
                })
            }
            crate::test_rpc::server::TestCommand::SetTab(tab) => {
                match tab.as_str() {
                    "info" => self.set_tab(DetailTab::Info),
                    "log" => self.set_tab(DetailTab::GitLog),
                    "change" => self.set_tab(DetailTab::GitChange),
                    "stash" => self.set_tab(DetailTab::GitStash),
                    _ => {}
                }
                None
            }
            crate::test_rpc::server::TestCommand::SetLogView(view) => {
                match view.as_str() {
                    "list" => self.set_log_view_mode(LogViewMode::List),
                    "canvas" => self.set_log_view_mode(LogViewMode::Canvas),
                    _ => {}
                }
                None
            }
        }
    }

    #[cfg(feature = "test-rpc")]
    pub fn apply_test_detail(&mut self, result: TestDetailResult) {
        self.apply_detail(
            result.request,
            result.detail,
            result.submodule_detail,
            result.log_entries,
            result.commit_canvas_layout,
        );
    }

    #[cfg(feature = "test-rpc")]
    pub fn schedule_test_view_tree_publish(&mut self, cx: &mut Context<'_, Self>) {
        self.test_view_revision += 1;
        let revision = self.test_view_revision;
        let snapshot = self.test_view_tree_snapshot();
        let provider = self.tree_provider.clone();
        self.test_view_tree_task = Some(cx.background_executor().spawn(async move {
            let tree = snapshot.build();
            crate::test_rpc::server::publish_tree(&provider, revision, tree);
        }));
    }
}

#[cfg(feature = "test-rpc")]
pub struct TestDetailRequest {
    request: DetailRequest,
    path: PathBuf,
    submodule_detail: Option<SubmoduleDetail>,
    is_initialized: bool,
    graph_path: Option<PathBuf>,
}

#[cfg(feature = "test-rpc")]
pub struct TestDetailResult {
    request: DetailRequest,
    detail: Option<RepoDetail>,
    submodule_detail: Option<SubmoduleDetail>,
    log_entries: Vec<LogEntry>,
    commit_canvas_layout: Option<CommitCanvasLayout>,
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
            request: self.request,
            detail,
            submodule_detail: self.submodule_detail,
            log_entries,
            commit_canvas_layout: self
                .graph_path
                .as_deref()
                .and_then(|path| crate::ui::commit_canvas::load_layout_for_path(path, 200)),
        }
    }
}

impl Render for GitMasterApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        #[cfg(feature = "test-rpc")]
        {
            self.bounds_registry.borrow_mut().clear();
        }

        for kind in crate::ui::popup::PopupKind::ALL {
            self.popup_focus
                .entry(kind)
                .or_insert_with(|| cx.focus_handle());
        }
        self.narrow_window = f32::from(window.viewport_size().width) < 1250.0;
        self.ensure_search_inputs(cx);
        if self.blocked_action.is_some() && self.blocked_focus.is_none() {
            let focus = cx.focus_handle();
            window.focus(&focus);
            self.blocked_focus = Some(focus);
        }
        let top_bar = self.render_top_bar(window, cx);
        let repo_list = self.render_repo_list(window, cx);
        let detail_panel = self.render_detail_panel(window, cx);
        let context_menu = self.render_context_menu(window, cx);

        let main_content = div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h(px(0.0))
            .overflow_hidden()
            .child(self.track("repo-list-panel", repo_list))
            .children(detail_panel.map(|p| self.track("detail-panel", p)));
        let main_content = self.track("main-content", main_content);

        let root = div()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && this.dismiss_open_popup(window) {
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                if event.keystroke.key == "k" && event.keystroke.modifiers.control {
                    if let Some(input) = &this.repo_search {
                        window.focus(&input.read(cx).focus_handle(cx));
                    }
                    cx.stop_propagation();
                    return;
                }
                if event.keystroke.key == "tab" {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev();
                    } else {
                        window.focus_next();
                    }
                    cx.stop_propagation();
                }
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(theme::palette().bg_base))
            .text_color(rgb(theme::palette().text_primary))
            .font_family(if cfg!(windows) {
                "Segoe UI"
            } else {
                "sans-serif"
            })
            .text_size(px(16.0))
            .child(self.track("top-bar", top_bar))
            .child(main_content)
            .children(self.render_task_status(cx))
            .children(context_menu.map(|m| self.track("context-menu", m)))
            .children(self.render_blocked_action(cx));
        let root = crate::ui::focus_visibility::track_input(root);

        #[cfg(feature = "test-rpc")]
        {
            let entity = cx.entity();
            // Publish after this frame's prepaint has recorded all control bounds.
            // An idle X11 window may never deliver a subsequent frame callback.
            crate::test_rpc::tracked::tracked("root", root, &self.bounds_registry)
                .after_paint(move |cx| {
                    entity.update(cx, |this, cx| this.schedule_test_view_tree_publish(cx));
                })
                .into_any_element()
        }
        #[cfg(not(feature = "test-rpc"))]
        self.track("root", root)
    }
}

#[cfg(test)]
#[path = "app_state_tests.rs"]
mod tests;
