use super::focus_visibility::KeyboardFocusStyle;
use super::theme;
use crate::{app_state::GitMasterApp, batch_ops::BatchHost};
use gpui::*;
impl GitMasterApp {
    pub(super) fn render_batch_results(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(result) = &self.batch_result else {
            return div().into_any_element();
        };
        let action = result.action;
        let failed = result
            .items
            .iter()
            .filter(|i| i.outcome.is_err())
            .cloned()
            .collect::<Vec<_>>();
        let copied = result
            .items
            .iter()
            .map(|i| {
                format!(
                    "{}\t{}\t{}",
                    i.path.display(),
                    i.remote.as_deref().unwrap_or("—"),
                    if i.skipped {
                        "跳过：未配置远端"
                    } else {
                        i.outcome
                            .as_ref()
                            .map(|_| "成功")
                            .unwrap_or_else(|e| e.as_str())
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let outcome = if result.failed > 0 {
            if result.succeeded > 0 {
                "部分失败"
            } else {
                "失败"
            }
        } else if result.succeeded == 0 {
            "跳过"
        } else {
            "成功"
        };
        let mut header = div()
            .flex()
            .items_center()
            .gap(px(16.0))
            .h(px(52.0))
            .flex_shrink_0()
            .child(
                div()
                    .text_size(px(20.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!(
                        "工作区任务 · {}",
                        match action {
                            crate::batch_ops::BatchAction::Fetch => "Fetch All",
                            _ => "切换默认分支",
                        }
                    )),
            )
            .child(
                div()
                    .text_color(rgb(if result.failed > 0 {
                        theme::palette().yellow
                    } else {
                        theme::palette().green
                    }))
                    .child(outcome),
            )
            .child(div().flex_1().text_size(px(16.0)).child(format!(
                "成功 {} · 失败 {} · 跳过 {} · 共 {} 项",
                result.succeeded,
                result.failed,
                result.skipped,
                result.items.len()
            )));
        if !failed.is_empty() {
            header = header.child(self.track(
                "batch-retry-failed",
                batch_button("batch-retry-failed", "仅重试失败项").on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.start_batch_host(BatchHost::retry(action, failed.clone()), action, cx)
                    },
                )),
            ));
        }
        header = header
            .child(self.track(
                "batch-copy",
                batch_button("batch-copy", "复制结果").on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))
                }),
            ))
            .child(self.track(
                "batch-collapse",
                batch_button("batch-collapse", "收起").on_click(cx.listener(|this, _, _, cx| {
                    this.status_expanded = false;
                    cx.notify();
                })),
            ));
        let mut body = div()
            .id("batch-results")
            .flex()
            .flex_col()
            .max_h(px(360.0))
            .overflow_y_scroll()
            .overflow_x_scroll()
            .px(px(18.0))
            .border_t_1()
            .border_color(rgb(theme::palette().border))
            .child(header)
            .child(
                div()
                    .flex()
                    .h(px(30.0))
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(theme::palette().border))
                    .text_size(px(15.0))
                    .child(div().w(px(340.0)).child("仓库"))
                    .child(div().w(px(150.0)).child("远端"))
                    .child(div().w(px(140.0)).child("状态"))
                    .child("结果"),
            );
        let mut items = result.items.iter().enumerate().collect::<Vec<_>>();
        items.sort_by_key(|(_, i)| (i.outcome.is_ok(), i.skipped));
        for (index, item) in items {
            let expanded = self.batch_expanded.contains(&index);
            let failed = item.outcome.is_err();
            let state = if item.skipped {
                "跳过"
            } else if failed {
                "失败"
            } else {
                "成功"
            };
            let color = if item.skipped {
                theme::palette().text_subtle
            } else if failed {
                theme::palette().red
            } else {
                theme::palette().green
            };
            let details_id = format!("batch-details-{index}");
            let open_id = format!("batch-open-{index}");
            let retry_id = format!("batch-retry-{index}");
            let path = item.path.clone();
            let open_path = path.clone();
            let retry = item.clone();
            let mut row = div()
                .flex()
                .items_center()
                .min_w(px(780.0))
                .h(px(34.0))
                .gap(px(12.0))
                .border_b_1()
                .border_color(rgb(theme::palette().border))
                .text_size(px(16.0));
            row = row
                .child(
                    self.track(
                        &details_id,
                        div()
                            .id(ElementId::Name(details_id.clone().into()))
                            .w(px(328.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .focusable()
                            .tab_stop(true)
                            .cursor_pointer()
                            .children(failed.then(|| {
                                super::icons::icon(
                                    if expanded {
                                        "chevron-down"
                                    } else {
                                        "chevron-right"
                                    },
                                    16.0,
                                )
                            }))
                            .child(super::icons::icon("folder", 18.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(
                                        path.file_name()
                                            .unwrap_or(path.as_os_str())
                                            .to_string_lossy()
                                            .into_owned(),
                                    ),
                            )
                            .tooltip(super::tooltip::text(path.display().to_string()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if failed {
                                    if !this.batch_expanded.remove(&index) {
                                        this.batch_expanded.insert(index);
                                    }
                                }
                                cx.notify();
                            })),
                    ),
                )
                .child(
                    div()
                        .w(px(138.0))
                        .flex_shrink_0()
                        .child(item.remote.clone().unwrap_or_else(|| "—".into())),
                )
                .child(
                    div()
                        .w(px(128.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .text_color(rgb(color))
                        .child(
                            super::icons::icon(
                                if item.skipped {
                                    "minus"
                                } else if failed {
                                    "x"
                                } else {
                                    "check"
                                },
                                18.0,
                            )
                            .text_color(rgb(color)),
                        )
                        .child(state),
                )
                .child(div().flex_1().min_w_0().child(if item.skipped {
                    "未配置远端"
                } else if failed {
                    "操作失败 · 展开查看原因"
                } else {
                    "已完成"
                }));
            body = body.child(row);
            if expanded && let Err(error) = &item.outcome {
                body = body.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .px(px(32.0))
                        .py(px(8.0))
                        .bg(rgb(theme::palette().bg_elevated))
                        .child(
                            div()
                                .id(ElementId::Name(format!("batch-error-{index}").into()))
                                .flex_1()
                                .min_w_0()
                                .max_h(px(68.0))
                                .overflow_y_scroll()
                                .text_size(px(14.0))
                                .font_family("Consolas")
                                .child(crate::operation_log::redact(error)),
                        )
                        .child(self.track(
                            &open_id,
                            batch_button(open_id.clone(), "打开仓库").on_click(cx.listener(
                                move |this, _, _, cx| {
                                    if let Some(index) =
                                        this.repos.iter().position(|r| r.path == open_path)
                                    {
                                        this.begin_select(index);
                                        this.reload_current_detail(cx);
                                        cx.notify();
                                    }
                                },
                            )),
                        ))
                        .child(self.track(
                            &retry_id,
                            batch_button(retry_id.clone(), "重试").on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.start_batch_host(
                                        BatchHost::retry(action, vec![retry.clone()]),
                                        action,
                                        cx,
                                    )
                                },
                            )),
                        )),
                );
            }
        }
        self.track("batch-results", body).into_any_element()
    }
    pub(super) fn confirm_batch_switch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.scanning || self.repos.is_empty() {
            return;
        }
        let paths = self
            .repos
            .iter()
            .map(|r| r.path.clone())
            .collect::<Vec<_>>();
        let parent = self.parent_dir.clone();
        self.busy = true;
        self.workspace_menu_open = false;
        self.set_status("正在读取批量切换预览…");
        cx.notify();
        let entity = cx.entity().downgrade();
        self.remote_action_prompt_task=Some(window.spawn(cx,async move|cx|{
            let preview_paths=paths.clone();
            let table=cx.background_executor().spawn(async move{preview_paths.iter().map(|path|{
                let (target, state) = match git2::Repository::open(path) {
                    Ok(repo) => match crate::repo_actions::main_branch(&repo) {
                        Ok((branch, _)) => {
                            let state = crate::repo_actions::switch_preview_state(&repo, &branch);
                            (branch, state)
                        },
                        Err(e) => ("—".into(), e),
                    },
                    Err(e) => ("—".into(), e.to_string()),
                };
                vec![path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned(), target, state]
            }).collect::<Vec<_>>()}).await;
            let description=super::confirmation::details("arrows-exchange",vec![],vec!["仅影响列出的父仓库，不递归切换子模块。执行时重新校验 Git 状态；不会强制覆盖本地修改。".into()],None,table);
            let answer=cx.prompt(PromptLevel::Warning,"切换到默认分支",Some(&description),&[PromptButton::cancel("取消"),PromptButton::ok("切换可执行项")]).await;
            entity.update(cx,|this,cx|{this.busy=false;if answer==Ok(1)&&this.parent_dir==parent{this.start_batch_host(BatchHost::new(paths,crate::batch_ops::BatchAction::SwitchMain),crate::batch_ops::BatchAction::SwitchMain,cx);}else{this.set_skipped("已取消批量切换");cx.notify();}}).ok();
        }));
    }
}

fn batch_button(id: impl Into<SharedString>, label: &str) -> Stateful<Div> {
    div()
        .id(ElementId::Name(id.into()))
        .px(px(12.0))
        .py(px(5.0))
        .border_1()
        .border_color(rgb(theme::palette().border))
        .rounded(px(3.0))
        .text_size(px(15.0))
        .focusable()
        .tab_stop(true)
        .keyboard_focus(|s| s.border_color(rgb(theme::palette().accent)))
        .hover(|s| s.bg(rgb(theme::palette().bg_elevated)))
        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
        .cursor_pointer()
        .child(label.to_owned())
}
