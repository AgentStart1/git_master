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
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(theme::BG_BASE))
            .text_color(rgb(theme::TEXT_PRIMARY))
            .child(self.render_top_bar(window, cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_grow()
                    .child(self.render_repo_list(window, cx))
                    .children(self.render_detail_panel(window, cx)),
            )
            .children(self.render_context_menu(window, cx))
    }
}
