use std::path::PathBuf;

use gpui::*;

use crate::git_ops;
use crate::models::{LogEntry, RepoDetail, RepoInfo};
use crate::ui::theme;

#[derive(Clone, Copy, PartialEq)]
pub enum DetailTab {
    Info,
    GitLog,
}

pub struct GitMasterApp {
    pub parent_dir: Option<PathBuf>,
    pub repos: Vec<RepoInfo>,
    pub selected_index: Option<usize>,
    pub active_tab: DetailTab,
    pub detail: Option<RepoDetail>,
    pub log_entries: Vec<LogEntry>,
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
        }
    }

    pub fn set_parent_dir(&mut self, path: PathBuf) {
        self.repos = git_ops::scan_repos(&path);
        self.parent_dir = Some(path);
        self.selected_index = None;
        self.detail = None;
        self.log_entries.clear();
    }

    pub fn select_repo(&mut self, index: usize) {
        self.selected_index = Some(index);
        self.active_tab = DetailTab::Info;
        let repo = &self.repos[index];
        self.detail = git_ops::get_repo_detail(&repo.path);
        self.log_entries = git_ops::get_commit_log(&repo.path, 200);
    }

    pub fn set_tab(&mut self, tab: DetailTab) {
        self.active_tab = tab;
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
    }
}
