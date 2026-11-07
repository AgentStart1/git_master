use super::focus_visibility::KeyboardFocusStyle;
use super::{icons::icon, theme};
use crate::{
    app_state::{DetailTab, GitMasterApp},
    models::RepoDetail,
};
use gpui::*;

type Comparison = (String, Option<(usize, usize)>);
fn reference_owner<'a>(
    reference: &str,
    remotes: &'a [crate::models::RemoteInfo],
) -> Option<&'a str> {
    remotes
        .iter()
        .filter(|r| reference.starts_with(&format!("{}/", r.name)))
        .max_by_key(|r| r.name.len())
        .map(|r| r.name.as_str())
}
fn comparison_reference<'a>(
    refs: &'a [Comparison],
    remotes: &[crate::models::RemoteInfo],
    upstream: Option<&str>,
    branch: &str,
    requested: Option<&str>,
    remote: Option<&str>,
) -> Option<&'a Comparison> {
    requested
        .and_then(|name| refs.iter().find(|(n, _)| n == name))
        .or_else(|| {
            remote.and_then(|remote| {
                refs.iter()
                    .find(|(n, _)| n == &format!("{remote}/{branch}"))
                    .or_else(|| {
                        refs.iter()
                            .find(|(n, _)| reference_owner(n, remotes) == Some(remote))
                    })
            })
        })
        .or_else(|| {
            if remote.is_none() {
                upstream
                    .and_then(|name| refs.iter().find(|(n, _)| n == name))
                    .or_else(|| refs.first())
            } else {
                None
            }
        })
}

impl GitMasterApp {
    pub(super) fn render_info_tab(
        &self,
        detail: &RepoDetail,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let repo = self
            .repos
            .iter()
            .enumerate()
            .find(|(_, repo)| repo.path.to_string_lossy() == detail.path);
        let reference = comparison_reference(
            &detail.comparison_refs,
            &detail.remotes,
            detail.upstream.as_deref(),
            &detail.current_branch,
            self.overview_reference.as_deref(),
            self.overview_remote.as_deref(),
        );
        let comparison_remote =
            reference.and_then(|(name, _)| reference_owner(name, &detail.remotes));
        let comparison = reference.and_then(|(_, counts)| *counts);
        let path = detail.path.clone();
        let copy_path = path.clone();
        let head = detail.head_oid.clone().unwrap_or_else(|| "尚无提交".into());
        let copy_head = head.clone();
        let mut info = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.0))
            .p(px(22.0))
            .gap(px(10.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(div().flex_1().text_size(px(17.0)).child(format!(
                        "{} · Git 状态：{}",
                        if detail.file_status.new_files
                            + detail.file_status.modified
                            + detail.file_status.deleted
                            + detail.file_status.renamed
                            + detail.file_status.conflicted
                            > 0
                        {
                            "工作区有修改"
                        } else {
                            "工作区干净"
                        },
                        detail.git_state
                    )))
                    .child(self.track(
                        "overview-refresh",
                        button("overview-refresh", "刷新").on_click(cx.listener(
                            |this, _, _, cx| {
                                if !this.busy && !this.loading_detail {
                                    this.reload_current_detail(cx);
                                    cx.notify();
                                }
                            },
                        )),
                    )),
            )
            .child(heading("folder", "仓库信息"))
            .child(
                line("本地路径", path.clone())
                    .child(self.track(
                        "overview-copy-path",
                        button("overview-copy-path", "复制").on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy_path.clone()))
                        }),
                    ))
                    .child(self.track(
                        "overview-open-path",
                        button("overview-open-path", "打开").on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.open_overview_location(path.clone().into(), false, cx)
                            },
                        )),
                    )),
            )
            .child(line("当前分支", detail.current_branch.clone()))
            .child(
                line("HEAD", head).child(button("overview-copy-head", "复制").on_click(
                    move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy_head.clone()))
                    },
                )),
            )
            .child(heading("file", "工作区变更"));
        let status = &detail.file_status;
        info = info
            .child(
                div().flex().flex_wrap().gap(px(20.0)).children(
                    [
                        ("新增", status.new_files),
                        ("修改", status.modified),
                        ("删除", status.deleted),
                        ("重命名", status.renamed),
                        ("冲突", status.conflicted),
                    ]
                    .into_iter()
                    .map(|(label, n)| div().text_size(px(16.0)).child(format!("{label}  {n}"))),
                ),
            )
            .child(self.track(
                "overview-changes",
                button("overview-changes", "查看变更 →").on_click(cx.listener(|this, _, _, cx| {
                    this.set_tab(DetailTab::GitChange);
                    this.run_worktree(crate::worktree::Action::Refresh, cx);
                    cx.notify();
                })),
            ))
            .child(heading("cloud", "远端仓库"));
        if detail.remotes.is_empty() {
            info = info.child(note("尚未配置远端仓库；同步操作不可用。"));
        }
        if !detail.remotes.is_empty() {
            info = info.child(
                div()
                    .flex()
                    .h(px(30.0))
                    .items_center()
                    .gap(px(8.0))
                    .bg(rgb(theme::palette().bg_elevated))
                    .text_size(px(14.0))
                    .child(div().w(px(72.0)).flex_shrink_0().child("远端"))
                    .child(div().flex_1().child("地址"))
                    .child(div().w(px(58.0)).flex_shrink_0().child("领先"))
                    .child(div().w(px(58.0)).flex_shrink_0().child("落后"))
                    .child(div().w(px(88.0)).flex_shrink_0().child("最近获取"))
                    .child(div().w(px(132.0)).flex_shrink_0().child("操作")),
            );
        }
        for remote in &detail.remotes {
            let name = remote.name.clone();
            let sync = repo
                .and_then(|(_, r)| r.remote_statuses.iter().find(|s| s.name == name))
                .and_then(|s| s.counts);
            let selected = comparison_remote == Some(name.as_str());
            let select_id = format!("overview-remote-{name}");
            let fetch_id = format!("overview-fetch-{name}");
            let fetch_name = name.clone();
            let url = remote
                .url
                .as_ref()
                .map(|url| crate::operation_log::redact(url))
                .unwrap_or_else(|| "未设置地址".into());
            info = info.child(
                div()
                    .flex()
                    .items_center()
                    .h(px(42.0))
                    .border_b_1()
                    .border_color(rgb(theme::palette().border))
                    .gap(px(8.0))
                    .text_size(px(15.0))
                    .child(div().w(px(72.0)).flex_shrink_0().child(name.clone()))
                    .child(
                        div()
                            .id(ElementId::Name(format!("overview-url-{name}").into()))
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .tooltip(super::tooltip::text(url.clone()))
                            .child(url),
                    )
                    .child(
                        div()
                            .w(px(58.0))
                            .flex_shrink_0()
                            .text_color(rgb(theme::palette().accent))
                            .child(
                                sync.map(|(a, _)| format!("↑ {a}"))
                                    .unwrap_or_else(|| "未比较".into()),
                            ),
                    )
                    .child(
                        div()
                            .w(px(58.0))
                            .flex_shrink_0()
                            .text_color(rgb(theme::palette().yellow))
                            .child(
                                sync.map(|(_, b)| format!("↓ {b}"))
                                    .unwrap_or_else(|| "未获取".into()),
                            ),
                    )
                    .child(
                        div().w(px(88.0)).flex_shrink_0().text_size(px(14.0)).child(
                            self.last_fetch
                                .get(&(std::path::PathBuf::from(&detail.path), remote.name.clone()))
                                .cloned()
                                .unwrap_or_else(|| "未记录".into()),
                        ),
                    )
                    .child(
                        self.track(
                            &select_id,
                            button(select_id.clone(), if selected { "比较中" } else { "比较" })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.overview_remote = Some(name.clone());
                                    this.overview_reference = None;
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(self.track(
                        &fetch_id,
                        button(fetch_id.clone(), "Fetch").on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.do_fetch_selected_remote(fetch_name.clone(), cx)
                            },
                        )),
                    )),
            );
        }
        info = info.child(note(
            "领先/落后基于最近 Fetch 的本地引用，不代表实时远端状态。",
        ));
        info = info.child(heading("stack-2", "子模块"));
        if let Some((repo_index, repo)) = repo {
            info = info.child(
                div()
                    .flex()
                    .h(px(30.0))
                    .items_center()
                    .bg(rgb(theme::palette().bg_elevated))
                    .text_size(px(14.0))
                    .child(div().flex_1().child("子模块"))
                    .child(div().w(px(110.0)).child("状态"))
                    .child(div().w(px(100.0)).child("分支"))
                    .child(div().w(px(86.0)).child("操作")),
            );
            if repo.submodules.is_empty() {
                info = info.child(note("没有子模块"));
            }
            for (module_index, module) in repo.submodules.iter().enumerate() {
                let relative = module.relative_path.clone();
                let initialized = module.is_initialized;
                let id = format!("overview-submodule-{module_index}");
                info = info.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .py(px(10.0))
                        .border_b_1()
                        .border_color(rgb(theme::palette().border))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(16.0))
                                .child(module.name.clone()),
                        )
                        .child(div().w(px(100.0)).child(note(if initialized {
                            "已初始化"
                        } else {
                            "未初始化"
                        })))
                        .child(div().w(px(90.0)).child(note(if initialized {
                            &module.current_branch
                        } else {
                            "—"
                        })))
                        .child(
                            self.track(
                                &id,
                                button(
                                    id.clone(),
                                    if initialized {
                                        "查看"
                                    } else {
                                        "查看并初始化"
                                    },
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.begin_select_submodule(
                                            repo_index,
                                            module_index,
                                            relative.clone(),
                                        );
                                        this.reload_current_detail(cx);
                                        cx.notify();
                                    },
                                )),
                            ),
                        ),
                );
            }
        } else {
            info = info.child(note("当前正在查看子模块仓库。"));
        }
        let popup = super::popup::PopupBoundary::new(self, super::popup::PopupKind::Reference);
        let right = div()
            .flex()
            .flex_col()
            .w(px(if self.narrow_window { 250.0 } else { 330.0 }))
            .flex_shrink_0()
            .border_l_1()
            .border_color(rgb(theme::palette().border))
            .p(px(20.0))
            .gap(px(18.0))
            .child(heading("git-branch", "分支与同步"))
            .child(note("比较分支"))
            .child(
                popup.attach(
                    div()
                        .relative()
                        .child(
                            self.track(
                                "overview-reference-picker",
                                button(
                                    "overview-reference-picker",
                                    reference
                                        .map(|(name, _)| name.as_str())
                                        .unwrap_or("无可比较引用 / 未获取"),
                                )
                                .track_focus(&popup.focus)
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.overview_ref_picker = !this.overview_ref_picker;
                                        cx.notify();
                                    },
                                )),
                            ),
                        )
                        .children(self.overview_ref_picker.then(|| {
                            deferred(
                                popup.observe(
                                    div()
                                        .id("overview-reference-options")
                                        .absolute()
                                        .top(px(36.0))
                                        .right_0()
                                        .w(px(288.0))
                                        .max_h(px(260.0))
                                        .overflow_y_scroll()
                                        .occlude()
                                        .bg(rgb(theme::palette().bg_surface))
                                        .border_1()
                                        .border_color(rgb(theme::palette().border))
                                        .p(px(8.0))
                                        .children(detail.comparison_refs.iter().map(
                                            |(name, _)| {
                                                let selected = name.clone();
                                                let selected_remote =
                                                    reference_owner(name, &detail.remotes)
                                                        .map(str::to_owned);
                                                let id = format!("overview-reference-{name}");
                                                self.track(
                                                    &id,
                                                    button(id.clone(), name).on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            this.overview_reference =
                                                                Some(selected.clone());
                                                            this.overview_remote =
                                                                selected_remote.clone();
                                                            this.close_popup(
                                                                super::popup::PopupKind::Reference,
                                                                window,
                                                            );
                                                            cx.notify();
                                                        },
                                                    )),
                                                )
                                            },
                                        )),
                                ),
                            )
                        })),
                    cx,
                ),
            )
            .child(note(
                &reference
                    .map(|(name, _)| format!("{} ↔ {}", detail.current_branch, name))
                    .unwrap_or_else(|| {
                        if detail.remotes.is_empty() {
                            "无远端 · 未比较"
                        } else {
                            "已配置远端 · 无可比较引用"
                        }
                        .into()
                    }),
            ))
            .child(
                div()
                    .text_size(px(23.0))
                    .text_color(rgb(theme::sync_color(comparison)))
                    .child(match comparison {
                        Some((0, 0)) => "已同步".into(),
                        Some((a, b)) => format!("领先 {a} · 落后 {b}"),
                        None => "未比较 / 未获取".into(),
                    }),
            )
            .child(note(
                "基于最近一次 Fetch 的本地远端引用。选择比较远端不会修改上游配置。",
            ))
            .child(self.track(
                "overview-history",
                button("overview-history", "查看提交历史 →").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.set_tab(DetailTab::GitLog);
                        cx.notify();
                    },
                )),
            ))
            .child(heading("file", "操作提示"))
            .child(note(
                "Fetch 只更新远端引用。切换分支和 Reset 会重新校验本地修改及 Git 操作状态。",
            ));
        div()
            .id("repo-info-scroll")
            .flex()
            .flex_1()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .child(info)
            .child(right)
            .into_any_element()
    }
    pub(super) fn open_overview_location(
        &mut self,
        path: std::path::PathBuf,
        terminal: bool,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::desktop_actions::open(&path, terminal) })
                .await;
            if let Err(error) = result {
                entity
                    .update(cx, |this, cx| {
                        this.set_failure(format!("无法打开位置：{error}"));
                        cx.notify();
                    })
                    .ok();
            }
        })
        .detach();
    }
}
fn heading(name: &'static str, title: &'static str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .pt(px(8.0))
        .text_size(px(20.0))
        .font_weight(FontWeight::MEDIUM)
        .child(icon(name, 21.0))
        .child(title)
}
fn note(text: &str) -> Div {
    div()
        .text_size(px(15.0))
        .text_color(rgb(theme::palette().text_subtle))
        .child(text.to_owned())
}
fn line(label: &'static str, value: String) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .text_size(px(16.0))
        .child(
            div()
                .w(px(80.0))
                .flex_shrink_0()
                .flex_shrink_0()
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .text_ellipsis()
                .child(value),
        )
}
fn button(id: impl Into<SharedString>, label: &str) -> Stateful<Div> {
    div()
        .id(ElementId::Name(id.into()))
        .px(px(10.0))
        .py(px(4.0))
        .border_1()
        .border_color(rgb(theme::palette().border))
        .rounded(px(3.0))
        .text_size(px(15.0))
        .focusable()
        .tab_stop(true)
        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(theme::palette().bg_elevated)))
        .child(label.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn comparison_context_follows_actual_reference_and_never_borrows_another_remote() {
        let remotes = ["origin", "backup", "unfetched", "origin/team"].map(|name| {
            crate::models::RemoteInfo {
                name: name.into(),
                url: None,
            }
        });
        let refs = vec![
            ("origin/main".into(), Some((1, 0))),
            ("backup/main".into(), Some((1, 1))),
            ("origin/team/topic".into(), Some((2, 3))),
        ];
        let selected = comparison_reference(
            &refs,
            &remotes,
            Some("origin/main"),
            "main",
            Some("origin/main"),
            Some("backup"),
        )
        .unwrap();
        assert_eq!(selected.1, Some((1, 0)));
        assert_eq!(reference_owner(&selected.0, &remotes), Some("origin"));
        assert!(
            comparison_reference(
                &refs,
                &remotes,
                Some("origin/main"),
                "main",
                None,
                Some("unfetched")
            )
            .is_none()
        );
        let selected = comparison_reference(
            &refs,
            &remotes,
            Some("origin/main"),
            "main",
            None,
            Some("backup"),
        )
        .unwrap();
        assert_eq!(reference_owner(&selected.0, &remotes), Some("backup"));
        assert_eq!(selected.1, Some((1, 1)));
        assert_eq!(
            reference_owner("origin/team/topic", &remotes),
            Some("origin/team")
        );
    }
}
