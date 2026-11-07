use super::focus_visibility::KeyboardFocusStyle;
use super::{
    text_input::TextInput,
    theme::{self, ThemeMode},
};
use crate::{app_state::GitMasterApp, models::RepoInfo};
use gpui::*;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum RepoFilter {
    #[default]
    All,
    Modified,
    Sync,
}
pub fn needs_sync(repo: &RepoInfo) -> bool {
    repo.remote_statuses
        .iter()
        .any(|remote| remote.counts.is_some_and(|counts| counts != (0, 0)))
}
pub fn needs_attention(repo: &RepoInfo) -> bool {
    repo.is_dirty
        || needs_sync(repo)
        || repo
            .submodules
            .iter()
            .any(|s| s.is_dirty || !s.is_initialized)
}
pub fn matches(repo: &RepoInfo, query: &str, filter: RepoFilter) -> bool {
    let query = query.trim().to_lowercase();
    let search = query.is_empty()
        || repo.name.to_lowercase().contains(&query)
        || repo.path.to_string_lossy().to_lowercase().contains(&query)
        || repo.submodules.iter().any(|s| {
            s.name.to_lowercase().contains(&query)
                || s.relative_path
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&query)
        });
    search
        && match filter {
            RepoFilter::All => true,
            RepoFilter::Modified => repo.is_dirty || repo.submodules.iter().any(|s| s.is_dirty),
            RepoFilter::Sync => needs_sync(repo),
        }
}
impl GitMasterApp {
    pub(crate) fn ensure_search_inputs(&mut self, cx: &mut Context<Self>) {
        if self.branch_menu_focus.is_none() {
            self.branch_menu_focus = Some(cx.focus_handle());
        }
        for picker in [
            super::branch_filter::BranchPicker::Display,
            super::branch_filter::BranchPicker::Checkout,
        ] {
            self.branch_trigger_focus
                .entry(picker)
                .or_insert_with(|| cx.focus_handle());
        }
        if self.repo_search.is_none() {
            let input = cx.new(|cx| TextInput::new("搜索仓库（名称 / 路径）", cx));
            cx.observe(&input, |this, _, cx| {
                this.repo_scroll.set_offset(point(px(0.0), px(0.0)));
                cx.notify();
            })
            .detach();
            self.repo_search = Some(input);
        }
        if self.branch_search.is_none() {
            let input = cx.new(|cx| TextInput::new("搜索分支", cx));
            cx.observe(&input, |_, _, cx| cx.notify()).detach();
            self.branch_search = Some(input);
        }
    }
    pub(super) fn render_theme_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .children(
                [
                    ("theme-light", "浅色", ThemeMode::Light),
                    ("theme-dark", "深色", ThemeMode::Dark),
                ]
                .into_iter()
                .map(|(id, label, mode)| {
                    self.track(
                        id,
                        div()
                            .id(id)
                            .px(px(8.0))
                            .py(px(5.0))
                            .text_size(px(15.0))
                            .focusable()
                            .tab_stop(true)
                            .keyboard_focus(|s| {
                                s.border_1().border_color(rgb(theme::palette().accent))
                            })
                            .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                            .cursor_pointer()
                            .border_b_2()
                            .border_color(rgb(if theme::mode() == mode {
                                theme::palette().accent
                            } else {
                                theme::palette().bg_surface
                            }))
                            .text_color(rgb(if theme::mode() == mode {
                                theme::palette().text_primary
                            } else {
                                theme::palette().text_subtle
                            }))
                            .child(label)
                            .on_click(cx.listener(move |_, _, _, cx| {
                                theme::set_mode(mode);
                                cx.refresh_windows();
                                cx.background_executor()
                                    .spawn(async {
                                        if let Err(error) = crate::preferences::save_current() {
                                            let _ = crate::operation_log::append(&format!(
                                                "Cannot save appearance preference: {error}"
                                            ));
                                        }
                                    })
                                    .detach();
                                cx.notify();
                            })),
                    )
                }),
            )
            .into_any_element()
    }
    pub(super) fn render_repo_filters(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap(px(26.0))
            .children(
                [
                    ("repo-filter-all", "全部", RepoFilter::All),
                    ("repo-filter-modified", "有修改", RepoFilter::Modified),
                    ("repo-filter-sync", "待同步", RepoFilter::Sync),
                ]
                .into_iter()
                .map(|(id, label, filter)| {
                    let count = self
                        .repos
                        .iter()
                        .filter(|repo| matches(repo, "", filter))
                        .count();
                    self.track(
                        id,
                        div()
                            .id(id)
                            .py(px(6.0))
                            .text_size(px(15.0))
                            .focusable()
                            .tab_stop(true)
                            .keyboard_focus(|s| {
                                s.border_1().border_color(rgb(theme::palette().accent))
                            })
                            .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                            .cursor_pointer()
                            .border_b_2()
                            .border_color(rgb(if self.repo_filter == filter {
                                theme::palette().accent
                            } else {
                                theme::palette().bg_surface
                            }))
                            .text_color(rgb(if self.repo_filter == filter {
                                theme::palette().text_primary
                            } else {
                                theme::palette().text_subtle
                            }))
                            .child(format!("{label} {count}"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.repo_filter = filter;
                                this.repo_scroll.set_offset(point(px(0.0), px(0.0)));
                                cx.notify();
                            })),
                    )
                }),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RemoteStatus, SubmoduleInfo};
    fn repo() -> RepoInfo {
        RepoInfo {
            name: "Workspace".into(),
            path: "projects/workspace".into(),
            is_dirty: false,
            current_branch: "main".into(),
            submodules: Vec::new(),
            remote_statuses: Vec::new(),
        }
    }
    #[::core::prelude::v1::test]
    fn search_matches_parent_path_and_nested_module_without_case_sensitivity() {
        let mut repo = repo();
        repo.submodules.push(SubmoduleInfo {
            name: "Shared".into(),
            path: "projects/workspace/modules/shared".into(),
            relative_path: "modules/shared".into(),
            url: None,
            is_initialized: true,
            is_dirty: false,
            current_branch: "main".into(),
            remote_statuses: Vec::new(),
        });
        for query in [" WORKSPACE ", "PROJECTS/", "modules/shared", "SHARED"] {
            assert!(matches(&repo, query, RepoFilter::All), "{query}");
        }
        assert!(!matches(&repo, "absent", RepoFilter::All));
    }
    #[::core::prelude::v1::test]
    fn sync_filter_uses_fetched_comparison_and_combines_with_search() {
        let mut repo = repo();
        for (counts, expected) in [
            (None, false),
            (Some((0, 0)), false),
            (Some((1, 0)), true),
            (Some((0, 2)), true),
            (Some((2, 3)), true),
        ] {
            repo.remote_statuses = vec![RemoteStatus {
                name: "origin".into(),
                counts,
            }];
            assert_eq!(matches(&repo, "workspace", RepoFilter::Sync), expected);
            assert!(!matches(&repo, "other", RepoFilter::Sync));
        }
    }
    #[::core::prelude::v1::test]
    fn modified_filter_includes_dirty_submodules() {
        let mut repo = repo();
        assert!(!matches(&repo, "", RepoFilter::Modified));
        repo.is_dirty = true;
        assert!(matches(&repo, "", RepoFilter::Modified));
        repo.is_dirty = false;
        repo.submodules.push(SubmoduleInfo {
            name: "module".into(),
            path: "projects/workspace/module".into(),
            relative_path: "module".into(),
            url: None,
            is_initialized: true,
            is_dirty: true,
            current_branch: "main".into(),
            remote_statuses: Vec::new(),
        });
        assert!(matches(&repo, "", RepoFilter::Modified));
        assert!(needs_attention(&repo));
    }
}
