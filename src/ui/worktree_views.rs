use super::focus_visibility::KeyboardFocusStyle;
use super::{icons::icon, theme};
use crate::{
    app_state::GitMasterApp,
    worktree::{Action, Snapshot},
};
use gpui::*;
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum PreviewSection {
    #[default]
    All,
    Staged,
    Working,
    Untracked,
    Conflict,
}
impl PreviewSection {
    pub fn title(self) -> &'static str {
        match self {
            Self::All => "全部差异",
            Self::Staged => "已暂存",
            Self::Working => "工作区",
            Self::Untracked => "未跟踪",
            Self::Conflict => "冲突",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Staged => "Staged changes",
            Self::Working => "Working tree changes",
            Self::Untracked => "Untracked files",
            _ => "",
        }
    }
}
impl GitMasterApp {
    pub(super) fn render_worktree_tab(&self, stash: bool, cx: &mut Context<Self>) -> AnyElement {
        let snapshot = self.worktree.as_ref().and_then(|(_, r)| r.as_ref().ok());
        let count = snapshot.map_or(0, |s| {
            if stash {
                s.stashes.len()
            } else {
                s.changes.len()
            }
        });
        let mut toolbar = div()
            .flex()
            .items_center()
            .h(px(56.0))
            .flex_shrink_0()
            .px(px(18.0))
            .gap(px(12.0))
            .border_b_1()
            .border_color(rgb(theme::palette().border))
            .child(div().flex_1().text_size(px(18.0)).child(format!(
                "{}  {count}",
                if stash {
                    "暂存记录"
                } else {
                    "变更文件"
                }
            )))
            .children(stash.then(|| {
                self.worktree_button(
                    "stash-save".into(),
                    &format!("保存当前修改 ({})", snapshot.map_or(0, |s| s.changes.len())),
                    Action::SaveStash,
                    cx,
                )
            }))
            .child(self.worktree_button(
                "worktree-refresh".into(),
                if self.worktree_loading {
                    "读取中…"
                } else {
                    "刷新"
                },
                Action::Refresh,
                cx,
            ));
        if !stash && let Some((_, _, path, _, _)) = self.selected_remote_action_target() {
            let selected = self.worktree_selection().as_ref().map(|s| path.join(s));
            let directory = selected
                .and_then(|file| file.parent().map(std::path::Path::to_owned))
                .unwrap_or(path);
            toolbar = toolbar.child(
                self.track(
                    "changes-open-directory",
                    div()
                        .id("changes-open-directory")
                        .px(px(10.0))
                        .text_size(px(15.0))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .child("打开所在目录")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_overview_location(directory.clone(), false, cx)
                        })),
                ),
            );
        }
        let mut body = div()
            .id("worktree-content")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .children((stash || count == 0).then_some(toolbar));
        if let Some(detail) = self
            .detail
            .as_ref()
            .filter(|d| d.git_state != "正常" || d.file_status.conflicted > 0)
        {
            let path = std::path::PathBuf::from(&detail.path);
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .px(px(18.0))
                    .py(px(12.0))
                    .bg(rgb(theme::palette().bg_elevated))
                    .text_color(rgb(theme::palette().yellow))
                    .child(icon("alert-triangle", 22.0).text_color(rgb(theme::palette().yellow)))
                    .child(div().flex_1().text_size(px(15.0)).child(format!(
                        "{} · {} 个冲突文件。请在外部处理，完成后刷新；预览不会自动解决冲突。",
                        if detail.git_state.starts_with("Rebase") {
                            "Rebase 暂停"
                        } else if detail.git_state == "正常" {
                            "工作区存在冲突"
                        } else {
                            &detail.git_state
                        },
                        detail.file_status.conflicted
                    )))
                    .child(
                        self.track(
                            "conflict-terminal",
                            div()
                                .id("conflict-terminal")
                                .focusable()
                                .tab_stop(true)
                                .keyboard_focus(|s| {
                                    s.border_1().border_color(rgb(theme::palette().accent))
                                })
                                .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                .cursor_pointer()
                                .text_size(px(15.0))
                                .child("打开终端")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_overview_location(path.clone(), true, cx)
                                })),
                        ),
                    ),
            );
        }
        if let Some(Err(error)) = self.worktree.as_ref().map(|(_, r)| r) {
            body = body.child(
                div()
                    .px(px(18.0))
                    .py(px(8.0))
                    .text_color(rgb(theme::palette().red))
                    .child(error.clone()),
            );
        }
        let Some(snapshot) = snapshot else {
            return body
                .child(empty(
                    if self.worktree_loading {
                        "正在读取仓库"
                    } else {
                        "仓库读取失败"
                    },
                    if self.worktree_loading {
                        "请稍候…"
                    } else {
                        "点击刷新重试，或切换仓库。"
                    },
                ))
                .into_any_element();
        };
        if count == 0 && self.worktree_loading {
            return body
                .child(empty("正在读取仓库", "请稍候…"))
                .into_any_element();
        }
        if count == 0 {
            let panel = super::state_views::message(
                if stash { "stack-2" } else { "circle-check" },
                if stash {
                    "尚无 Stash 记录"
                } else {
                    "工作区没有修改"
                },
                if stash {
                    "当前修改可保存到 Stash，包含未跟踪文件。"
                } else {
                    "没有需要暂存或审阅的文件变更。"
                },
            )
            .child(if stash {
                self.worktree_button(
                    "empty-stash-save".into(),
                    "保存当前修改",
                    Action::SaveStash,
                    cx,
                )
            } else {
                self.worktree_button("empty-changes-refresh".into(), "刷新", Action::Refresh, cx)
            });
            return body.child(panel).into_any_element();
        }
        body.child(if stash {
            self.stash_content(snapshot, cx)
        } else {
            self.changes_content(snapshot, cx)
        })
        .into_any_element()
    }
    fn changes_content(&self, snapshot: &Snapshot, cx: &mut Context<Self>) -> AnyElement {
        let mut list = div()
            .id("worktree-list")
            .flex()
            .flex_col()
            .w(px(350.0))
            .flex_shrink_0()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .border_r_1()
            .border_color(rgb(theme::palette().border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(52.0))
                    .px(px(18.0))
                    .gap(px(8.0))
                    .border_b_1()
                    .border_color(rgb(theme::palette().border))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .text_size(px(17.0))
                            .child(format!("{} 个文件", snapshot.changes.len()))
                            .children(snapshot.change_lines.map(|(added, removed)| {
                                div()
                                    .flex()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_color(rgb(theme::palette().green))
                                            .child(format!("+{added}")),
                                    )
                                    .child(
                                        div()
                                            .text_color(rgb(theme::palette().red))
                                            .child(format!("−{removed}")),
                                    )
                            })),
                    )
                    .child(self.worktree_button(
                        "worktree-refresh".into(),
                        if self.worktree_loading {
                            "读取中…"
                        } else {
                            "刷新"
                        },
                        Action::Refresh,
                        cx,
                    )),
            );
        for section in [
            PreviewSection::Conflict,
            PreviewSection::Staged,
            PreviewSection::Working,
            PreviewSection::Untracked,
        ] {
            let files = snapshot
                .changes
                .iter()
                .enumerate()
                .filter(|(_, e)| match section {
                    PreviewSection::Conflict => e.status.contains("Conflicted"),
                    PreviewSection::Staged => e.staged && !e.status.contains("Conflicted"),
                    PreviewSection::Working => {
                        e.unstaged
                            && !e.status.contains("Untracked")
                            && !e.status.contains("Conflicted")
                    }
                    PreviewSection::Untracked => e.status.contains("Untracked"),
                    _ => false,
                })
                .collect::<Vec<_>>();
            if files.is_empty() {
                continue;
            }
            list = list.child(
                div()
                    .px(px(18.0))
                    .py(px(14.0))
                    .text_size(px(17.0))
                    .child(format!(
                        "{}  ({})",
                        if section == PreviewSection::Working {
                            "未暂存"
                        } else {
                            section.title()
                        },
                        files.len()
                    )),
            );
            for (index, entry) in files {
                let kind = if section == PreviewSection::Staged {
                    entry.index_kind
                } else {
                    entry.worktree_kind
                };
                let path = entry.path.clone();
                let selected = self.worktree_selection().as_deref()
                    == Some(&path.display().to_string())
                    && self.preview_section == section;
                let id = if section == PreviewSection::Staged {
                    format!("change-diff-staged-{index}")
                } else {
                    format!("change-diff-{index}")
                };
                let row = div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .h(px(44.0))
                    .px(px(16.0))
                    .border_b_1()
                    .border_color(rgb(theme::palette().border))
                    .bg(rgb(if selected {
                        theme::palette().accent_soft
                    } else {
                        theme::palette().bg_base
                    }))
                    .child(icon("file", 18.0))
                    .child(
                        div()
                            .text_color(rgb(
                                if matches!(
                                    kind,
                                    Some(
                                        crate::worktree::ChangeKind::Added
                                            | crate::worktree::ChangeKind::Untracked
                                    )
                                ) {
                                    theme::palette().green
                                } else {
                                    theme::palette().accent
                                },
                            ))
                            .child(kind.map_or("", crate::worktree::ChangeKind::badge)),
                    )
                    .child(
                        self.track(
                            &id,
                            div()
                                .id(ElementId::Name(id.clone().into()))
                                .flex_1()
                                .min_w(px(0.0))
                                .text_size(px(16.0))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .focusable()
                                .tab_stop(true)
                                .keyboard_focus(|s| {
                                    s.border_1().border_color(rgb(theme::palette().accent))
                                })
                                .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                .cursor_pointer()
                                .child(path.display().to_string())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.preview_section = section;
                                    this.run_worktree(Action::Diff(path.clone()), cx);
                                })),
                        ),
                    )
                    .child(if section == PreviewSection::Staged {
                        self.worktree_button(
                            format!("change-unstage-{index}"),
                            "取消暂存",
                            Action::Unstage(entry.path.clone()),
                            cx,
                        )
                    } else {
                        self.worktree_button(
                            format!("change-stage-{index}"),
                            if section == PreviewSection::Conflict {
                                "标记已解决"
                            } else {
                                "暂存"
                            },
                            Action::Stage(entry.path.clone()),
                            cx,
                        )
                    });
                list = list.child(row);
            }
        }
        let title = self
            .worktree_selection()
            .clone()
            .unwrap_or_else(|| "尚未选择文件".into());
        let mut preview = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .h(px(52.0))
                    .px(px(18.0))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(theme::palette().border))
                    .child(icon("file", 20.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(20.0))
                            .child(title),
                    )
                    .children(
                        self.worktree_selection()
                            .as_ref()
                            .and_then(|selected| {
                                snapshot
                                    .changes
                                    .iter()
                                    .enumerate()
                                    .find(|(_, e)| e.path.to_string_lossy() == selected.as_str())
                            })
                            .map(|(index, e)| {
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .px(px(7.0))
                                            .py(px(3.0))
                                            .border_1()
                                            .border_color(rgb(theme::palette().border))
                                            .child(
                                                if self.preview_section == PreviewSection::Staged {
                                                    "已暂存"
                                                } else if self.preview_section
                                                    == PreviewSection::Conflict
                                                {
                                                    "冲突"
                                                } else {
                                                    "未暂存"
                                                },
                                            ),
                                    )
                                    .child(if self.preview_section == PreviewSection::Staged {
                                        self.worktree_button(
                                            format!("selected-unstage-{index}"),
                                            "取消暂存",
                                            Action::Unstage(e.path.clone()),
                                            cx,
                                        )
                                    } else {
                                        self.worktree_button(
                                            format!("selected-stage-{index}"),
                                            if self.preview_section == PreviewSection::Conflict {
                                                "标记已解决"
                                            } else {
                                                "暂存文件"
                                            },
                                            Action::Stage(e.path.clone()),
                                            cx,
                                        )
                                    })
                            }),
                    ),
            )
            .child(
                div()
                    .px(px(18.0))
                    .py(px(8.0))
                    .text_size(px(16.0))
                    .text_color(rgb(theme::palette().text_subtle))
                    .child(match self.preview_section {
                        PreviewSection::Staged => "HEAD ↔ 暂存区",
                        PreviewSection::Working => "暂存区 ↔ 工作区",
                        PreviewSection::Untracked => "未跟踪文件 · 只读预览",
                        PreviewSection::Conflict => "冲突只读预览 · 请在外部处理后刷新",
                        _ => "选择文件查看差异",
                    }),
            );
        if let Some((_, _, path, _, _)) = self.selected_remote_action_target() {
            let directory = self
                .worktree_selection()
                .as_ref()
                .map(|s| path.join(s))
                .and_then(|p| p.parent().map(std::path::Path::to_owned))
                .unwrap_or(path);
            preview = preview.child(
                self.track(
                    "changes-open-directory",
                    div()
                        .id("changes-open-directory")
                        .px(px(18.0))
                        .py(px(5.0))
                        .text_size(px(13.0))
                        .text_color(rgb(theme::palette().accent))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .child("在外部查看 · 打开所在目录")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_overview_location(directory.clone(), false, cx)
                        })),
                ),
            );
        }
        if let Some((_, _, root, _, _)) = self.selected_remote_action_target()
            && let Some(selected) = &self.worktree_selection()
        {
            let file = root.join(selected);
            preview = preview.child(
                self.track(
                    "changes-open-file",
                    div()
                        .id("changes-open-file")
                        .px(px(18.0))
                        .py(px(6.0))
                        .text_color(rgb(theme::palette().accent))
                        .focusable()
                        .tab_stop(true)
                        .cursor_pointer()
                        .child("在外部打开此文件")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_overview_location(file.clone(), false, cx)
                        })),
                ),
            );
        }
        preview = preview.child(self.section_diff(snapshot, false, cx));
        preview = preview.child(
            div()
                .px(px(18.0))
                .py(px(10.0))
                .text_size(px(15.0))
                .border_t_1()
                .border_color(rgb(theme::palette().border))
                .child("文件内容只读 · 暂存不会丢弃本地修改"),
        );
        div()
            .flex()
            .flex_1()
            .min_h(px(0.0))
            .child(list)
            .child(preview)
            .into_any_element()
    }
    fn stash_content(&self, snapshot: &Snapshot, cx: &mut Context<Self>) -> AnyElement {
        let mut list = div()
            .id("worktree-list")
            .flex()
            .flex_col()
            .w(px(320.0))
            .flex_shrink_0()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .border_r_1()
            .border_color(rgb(theme::palette().border));
        for entry in &snapshot.stashes {
            let oid = entry.oid;
            let id = format!("stash-select-{oid}");
            let selected = self.worktree_selection().as_deref() == Some(&oid.to_string());
            list = list.child(
                self.track(
                    &id,
                    div()
                        .id(ElementId::Name(id.clone().into()))
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .p(px(18.0))
                        .border_b_1()
                        .border_color(rgb(theme::palette().border))
                        .bg(rgb(if selected {
                            theme::palette().accent_soft
                        } else {
                            theme::palette().bg_base
                        }))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .text_size(px(16.0))
                                .child(format!("stash@{{{}}}", entry.index))
                                .child(entry.date.split(' ').next().unwrap_or("").to_owned()),
                        )
                        .child(div().text_size(px(16.0)).child(entry.message.clone()))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(rgb(theme::palette().text_subtle))
                                .child(format!("分支 · {}", entry.branch)),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.preview_section = PreviewSection::Working;
                            this.run_worktree(Action::ShowStash(oid), cx);
                        })),
                ),
            );
        }
        let selected = snapshot
            .stashes
            .iter()
            .find(|e| self.worktree_selection().as_deref() == Some(&e.oid.to_string()));
        let mut right = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .px(px(14.0));
        if let Some(entry) = selected {
            let id = entry.oid;
            right = right
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(8.0))
                        .py(px(12.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(21.0))
                                .child(entry.message.clone()),
                        )
                        .child(self.worktree_button(
                            format!("stash-apply-{id}"),
                            "应用并保留",
                            Action::Apply(id),
                            cx,
                        ))
                        .child(self.worktree_button(
                            format!("stash-pop-{id}"),
                            "应用后移除",
                            Action::Pop(id),
                            cx,
                        ))
                        .child(self.worktree_button(
                            format!("stash-drop-{id}"),
                            "删除",
                            Action::Drop(id),
                            cx,
                        )),
                )
                .child(
                    div()
                        .text_size(px(15.0))
                        .text_color(rgb(theme::palette().text_subtle))
                        .child(format!(
                            "哈希 {}     作者 {}     时间 {}",
                            &id.to_string()[..7],
                            entry.author,
                            entry.date
                        )),
                )
                .child(
                    div()
                        .py(px(10.0))
                        .text_size(px(15.0))
                        .child("Apply 保留记录，Pop 仅在成功后移除记录。"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(theme::palette().text_subtle))
                        .child("文件预览 · 增删行按保存分区累计"),
                )
                .child(
                    div()
                        .id("stash-files")
                        .max_h(px(132.0))
                        .overflow_y_scroll()
                        .children(
                            self.worktree_preview
                                .snapshot()
                                .into_iter()
                                .flat_map(|s| s.preview_files.iter())
                                .map(|(path, added, removed)| {
                                    div()
                                        .flex()
                                        .items_center()
                                        .h(px(30.0))
                                        .border_b_1()
                                        .border_color(rgb(theme::palette().border))
                                        .text_size(px(15.0))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.0))
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .child(path.clone()),
                                        )
                                        .child(
                                            div()
                                                .w(px(50.0))
                                                .text_color(rgb(theme::palette().green))
                                                .child(format!("+{added}")),
                                        )
                                        .child(
                                            div()
                                                .w(px(40.0))
                                                .text_color(rgb(theme::palette().red))
                                                .child(format!("-{removed}")),
                                        )
                                }),
                        ),
                )
                .child(
                    div().flex().gap(px(18.0)).py(px(10.0)).children(
                        [
                            PreviewSection::Staged,
                            PreviewSection::Working,
                            PreviewSection::Untracked,
                        ]
                        .into_iter()
                        .map(|section| {
                            let id = format!("stash-section-{}", section.key());
                            self.track(
                                &id,
                                div()
                                    .id(ElementId::Name(
                                        format!("stash-section-{}", section.key()).into(),
                                    ))
                                    .px(px(10.0))
                                    .py(px(6.0))
                                    .text_size(px(17.0))
                                    .border_b_2()
                                    .border_color(rgb(if self.preview_section == section {
                                        theme::palette().accent
                                    } else {
                                        theme::palette().bg_base
                                    }))
                                    .focusable()
                                    .tab_stop(true)
                                    .keyboard_focus(|s| {
                                        s.border_1().border_color(rgb(theme::palette().accent))
                                    })
                                    .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                    .cursor_pointer()
                                    .child(section.title())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.preview_section = section;
                                        cx.notify();
                                    })),
                            )
                        }),
                    ),
                )
                .child(self.section_diff(snapshot, true, cx));
        } else {
            right = right.child(empty(
                "选择一条暂存记录",
                "在右侧查看保存内容、应用或删除记录。",
            ));
        }
        div()
            .flex()
            .flex_1()
            .min_h(px(0.0))
            .child(list)
            .child(right)
            .into_any_element()
    }
    fn section_diff(
        &self,
        _snapshot: &Snapshot,
        stash: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let snapshot = match &self.worktree_preview {
            crate::preview_state::PreviewState::Failed { error, .. } => {
                return empty("无法读取差异", error);
            }
            state => match state.snapshot() {
                Some(snapshot) => snapshot,
                None if self.worktree_selection().is_some() => {
                    return empty("正在读取差异", "请稍候…");
                }
                None => return empty("选择文件查看差异", "暂存操作不会丢弃工作区修改。"),
            },
        };
        if let Some(error) = &snapshot.preview_error {
            return empty("无法读取差异", error);
        }
        if self.worktree_selection().is_none() {
            return empty("选择文件查看差异", "暂存操作不会丢弃工作区修改。");
        }
        if self.preview_section == PreviewSection::Conflict {
            let mut panel = div()
                .id("conflict-preview-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col();
            if let Some(rows) = snapshot.sections.get("冲突文件 · 当前工作区内容（只读）")
            {
                panel = panel.child(
                    self.track(
                        "conflict-pane-current",
                        div()
                            .id("conflict-pane-current")
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .overflow_hidden()
                            .h(px((rows.len() as f32 * 24.0 + 12.0).min(260.0)))
                            .flex_shrink_0()
                            .child(super::diff_view::render(rows, false)),
                    ),
                );
            }
            for (key, label) in [
                (
                    "当前内容（索引 stage 2）",
                    "当前内容 · CURRENT（索引 stage 2）",
                ),
                (
                    "传入内容（索引 stage 3）",
                    "传入内容 · INCOMING（索引 stage 3）",
                ),
            ] {
                let collapsed = self.conflict_collapsed.contains(key);
                let id = format!("conflict-side-{key}");
                panel = panel.child(
                    self.track(
                        &id,
                        div()
                            .id(ElementId::Name(id.clone().into()))
                            .p(px(14.0))
                            .flex_shrink_0()
                            .focusable()
                            .tab_stop(true)
                            .keyboard_focus(|s| {
                                s.border_1().border_color(rgb(theme::palette().accent))
                            })
                            .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                            .cursor_pointer()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{}  {label}", if collapsed { "▸" } else { "▾" }))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.conflict_collapsed.remove(key) {
                                    this.conflict_collapsed.insert(key.into());
                                }
                                cx.notify();
                            })),
                    ),
                );
                if !collapsed && let Some(rows) = snapshot.sections.get(key) {
                    let pane_id = format!("conflict-pane-{key}");
                    panel = panel.child(
                        self.track(
                            &pane_id,
                            div()
                                .id(ElementId::Name(pane_id.clone().into()))
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .overflow_hidden()
                                .h(px((rows.len() as f32 * 24.0).min(150.0)))
                                .flex_shrink_0()
                                .child(super::diff_view::render(&rows[1..], false)),
                        ),
                    );
                }
            }
            return panel.child(div().p(px(18.0)).flex_shrink_0().border_t_1().border_color(rgb(theme::palette().border)).flex().flex_col().gap(px(10.0))
                .child("1   在外部编辑器或终端处理冲突，并检查保存后的文件。")
                .child("2   刷新状态，明确确认后使用“标记已解决”暂存文件。")
                .child("3   在终端继续或中止当前 Git 操作（例如 git rebase --continue / --abort）。")
                .child(div().text_size(px(13.0)).text_color(rgb(theme::palette().text_subtle)).child("双方内容来自未合并索引；Rebase 中 stage 2/3 的分支含义由 Git 决定，不据标记推断解决结果。"))).into_any_element();
        }
        let rows = if self.preview_section == PreviewSection::All
            || self.preview_section == PreviewSection::Conflict
        {
            snapshot.diff_rows.as_slice()
        } else {
            snapshot
                .sections
                .get(if stash {
                    self.preview_section.key()
                } else if self.preview_section == PreviewSection::Staged {
                    "Staged"
                } else {
                    "Working tree"
                })
                .map(Vec::as_slice)
                .unwrap_or(&[])
        };
        let binary = rows.iter().any(|r| {
            r.kind == crate::diff_model::RowKind::Note
                && r.text.to_ascii_lowercase().contains("binary")
        });
        let truncated = rows.iter().any(|r| {
            r.kind == crate::diff_model::RowKind::Note
                && ["truncated", "limited"]
                    .iter()
                    .any(|s| r.text.to_ascii_lowercase().contains(s))
        });
        if binary
            && !rows.iter().any(|r| {
                matches!(
                    r.kind,
                    crate::diff_model::RowKind::Added
                        | crate::diff_model::RowKind::Removed
                        | crate::diff_model::RowKind::Context
                )
            })
        {
            return super::state_views::message(
                "file",
                "二进制文件",
                "不提供文本预览。可使用“在外部打开此文件”查看。 ",
            )
            .into_any_element();
        }
        if truncated {
            return div().flex().flex_col().flex_1().min_h_0().child(div().p(px(14.0)).flex_shrink_0().bg(rgb(theme::palette().bg_elevated)).text_color(rgb(theme::palette().yellow)).child("预览已截断 · 最多 512 KiB 或 2000 行，当前仅显示有界内容。请在外部打开完整文件。"))
                .child(super::diff_view::render(rows,stash)).into_any_element();
        }
        if rows.is_empty() && self.worktree_selection().is_some() {
            return empty("该分区没有变更", "可切换其他分区或刷新仓库。");
        }
        super::diff_view::render(rows, stash)
    }
}
fn empty(title: &str, message: &str) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(14.0))
        .p(px(24.0))
        .child(div().text_size(px(21.0)).child(title.to_owned()))
        .child(
            div()
                .text_size(px(16.0))
                .text_color(rgb(theme::palette().text_subtle))
                .child(message.to_owned()),
        )
        .into_any_element()
}
