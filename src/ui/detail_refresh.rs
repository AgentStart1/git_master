use crate::app_state::GitMasterApp;
use crate::{git_ops, ui::commit_canvas};
use gpui::*;

impl GitMasterApp {
    // Re-read using the current selection/filter, not an operation's old UI snapshot.
    pub(super) fn reload_current_detail(&mut self, cx: &mut Context<'_, Self>) {
        let Some((_, _, path, _selection, submodule)) = self.selected_remote_action_target() else {
            return;
        };
        let request = self.begin_detail_request(path.clone());
        let branches = self.log_visible_branches.clone();
        let limit = self
            .commit_canvas_layout
            .as_ref()
            .map_or(200, |layout| layout.history_limit);
        let initialized = submodule
            .as_ref()
            .is_none_or(|module| module.is_initialized);
        self.loading_detail = true;
        self.detail_task = Some(cx.spawn(async move |entity, cx| {
            let (detail, layout) = cx
                .background_executor()
                .spawn(async move {
                    if !initialized {
                        return (None, None);
                    }
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
                        limit,
                    );
                    (detail, layout)
                })
                .await;
            entity
                .update(cx, |this, cx| {
                    if !this.owns_detail_request(&request) {
                        return;
                    }
                    this.apply_detail(request, detail, submodule, Vec::new(), layout);
                    this.refresh_visible_worktree(cx);
                    cx.notify();
                })
                .ok();
        }));
    }
}
