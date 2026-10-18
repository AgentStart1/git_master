use crate::app_state::GitMasterApp;
use crate::{git_ops, ui::commit_canvas};
use gpui::*;

impl GitMasterApp {
    // Re-read using the current selection/filter, not an operation's old UI snapshot.
    pub(super) fn reload_current_detail(&mut self, cx: &mut Context<'_, Self>) {
        let Some((_, _, path, selection, submodule)) = self.selected_remote_action_target() else {
            return;
        };
        let revision = self.navigation_revision;
        let branches = self.log_visible_branches.clone();
        self.loading_detail = true;
        self.detail_task = Some(cx.spawn(async move |entity, cx| {
            let expected_path = path.clone();
            let (detail, layout) = cx
                .background_executor()
                .spawn(async move {
                    let detail = git_ops::get_repo_detail(&path);
                    let branches = if branches.is_empty() {
                        detail
                            .as_ref()
                            .map(Self::default_log_visible_branches)
                            .unwrap_or_default()
                    } else {
                        branches
                    };
                    let layout = commit_canvas::load_layout_for_branches(
                        &path,
                        &branches.into_iter().collect::<Vec<_>>(),
                        200,
                    );
                    (detail, layout)
                })
                .await;
            entity
                .update(cx, |this, cx| {
                    if this.navigation_revision != revision
                        || !this.selected_remote_action_target().is_some_and(
                            |(_, _, path, current, _)| {
                                path == expected_path && current == selection
                            },
                        )
                    {
                        return;
                    }
                    this.apply_detail(selection, detail, submodule, Vec::new(), layout);
                    this.refresh_visible_worktree(cx);
                    cx.notify();
                })
                .ok();
        }));
    }
}
