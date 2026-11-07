use super::focus_visibility::KeyboardFocusStyle;
use super::icons::icon;
use gpui::*;

use crate::app_state::{GitMasterApp, RepoSelection};
use crate::git_ops;
use crate::models::SubmoduleDetail;
use crate::ui::{commit_canvas, theme};

fn repo_scrollbar_geometry(height: f32, max_offset: f32, offset: f32) -> (f32, f32) {
    let height = height.max(0.0);
    let thumb = if max_offset <= 0.0 {
        height
    } else {
        (height * height / (height + max_offset))
            .max(24.0)
            .min(height)
    };
    let top = if max_offset > 0.0 {
        (-offset / max_offset).clamp(0.0, 1.0) * (height - thumb)
    } else {
        0.0
    };
    (thumb, top)
}

#[cfg(test)]
mod scroll_tests {
    use super::repo_scrollbar_geometry;

    #[test]
    fn thumb_reaches_both_ends_and_handles_short_viewports() {
        assert_eq!(repo_scrollbar_geometry(100.0, 300.0, 0.0), (25.0, 0.0));
        assert_eq!(repo_scrollbar_geometry(100.0, 300.0, -300.0), (25.0, 75.0));
        assert_eq!(repo_scrollbar_geometry(10.0, 300.0, -300.0), (10.0, 0.0));
        assert_eq!(repo_scrollbar_geometry(100.0, 0.0, 0.0), (100.0, 0.0));
    }
}

impl GitMasterApp {
    pub fn render_repo_list(&self, _window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let query = self
            .repo_search
            .as_ref()
            .map(|input| input.read(cx).content.to_string())
            .unwrap_or_default();
        let ordered = [true, false]
            .into_iter()
            .flat_map(|attention| {
                self.repos.iter().enumerate().filter(move |(_, repo)| {
                    super::workspace_controls::needs_attention(repo) == attention
                })
            })
            .filter(|(_, repo)| super::workspace_controls::matches(repo, &query, self.repo_filter));
        let mut last_group = None;
        let repo_items: Vec<AnyElement> = ordered
            .flat_map(|(i, repo)| {
                let is_selected = self.selected == Some(RepoSelection::Repo(i));
                let is_expanded = self.expanded_repos.contains(&i);
                let has_submodules = !repo.submodules.is_empty();
                let bg = if is_selected {
                    rgb(theme::palette().accent_soft)
                } else {
                    rgb(theme::palette().bg_surface)
                };

                let dirty_color = if let Some(counts) = repo
                    .remote_statuses
                    .iter()
                    .filter_map(|s| s.counts)
                    .find(|c| *c != (0, 0))
                {
                    rgb(theme::sync_color(Some(counts)))
                } else if repo.is_dirty {
                    rgb(theme::palette().yellow)
                } else {
                    rgb(theme::palette().green)
                };

                let item = div()
                    .id(ElementId::Name(format!("repo-{i}").into()))
                    .flex_shrink_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .pl(px(if self.narrow_window { 12.0 } else { 26.0 }))
                    .pr(px(12.0))
                    .py(px(6.0))
                    .border_b_1()
                    .bg(bg)
                    .relative()
                    .border_b_1()
                    .border_color(rgb(theme::palette().border))
                    .children(is_selected.then(|| {
                        div()
                            .absolute()
                            .left(px(0.0))
                            .top(px(0.0))
                            .w(px(3.0))
                            .h_full()
                            .bg(rgb(theme::palette().accent))
                    }))
                    .tooltip(super::tooltip::text(format!(
                        "{}\n{}\n{}",
                        repo.name,
                        repo.path.display(),
                        repo.current_branch
                    )))
                    .focusable()
                    .tab_stop(true)
                    .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                    .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(theme::palette().bg_elevated)))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        let Some(repo) = this.repos.get(i).cloned() else {
                            return;
                        };
                        let path = repo.path.clone();
                        this.begin_select(i);
                        let request = this.begin_detail_request(path.clone());
                        cx.notify();
                        this.detail_task = Some(cx.spawn(async move |entity, cx| {
                            let (detail, log_entries, canvas_layout) = cx
                                .background_executor()
                                .spawn(async move {
                                    (
                                        git_ops::get_repo_detail(&path),
                                        git_ops::get_commit_log(&path, 200),
                                        commit_canvas::load_layout(&repo, 200),
                                    )
                                })
                                .await;
                            entity
                                .update(cx, |this, cx| {
                                    this.apply_detail(
                                        request,
                                        detail,
                                        None,
                                        log_entries,
                                        canvas_layout,
                                    );
                                    this.refresh_visible_worktree(cx);
                                    cx.notify();
                                })
                                .ok();
                        }));
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                            if this.repos.get(i).is_none() {
                                return;
                            }
                            this.open_context_menu(i, event.position);
                            cx.notify();
                        }),
                    )
                    .child(
                        self.track(
                            &format!("repo-{i}-toggle"),
                            div()
                                .id(ElementId::Name(format!("repo-{i}-toggle").into()))
                                .w(px(16.0))
                                .text_size(px(15.0))
                                .text_color(rgb(theme::palette().text_subtle))
                                .focusable()
                                .tab_stop(true)
                                .keyboard_focus(|s| {
                                    s.border_1().border_color(rgb(theme::palette().accent))
                                })
                                .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _event, _window, cx| {
                                    cx.stop_propagation();
                                    if this
                                        .repos
                                        .get(i)
                                        .is_some_and(|repo| !repo.submodules.is_empty())
                                    {
                                        this.toggle_repo_expanded(i);
                                        cx.notify();
                                    }
                                }))
                                .child(if has_submodules {
                                    if is_expanded { "▾" } else { "▸" }
                                } else {
                                    ""
                                }),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_x_hidden()
                            .gap(px(3.0))
                            .child(
                                div()
                                    .text_size(px(19.0))
                                    .flex()
                                    .items_center()
                                    .gap(px(12.0))
                                    .child(icon("folder", 20.0))
                                    .child(
                                        div()
                                            .min_w(px(0.0))
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .child(repo.name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .text_color(rgb(theme::palette().text_subtle))
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .pl(px(32.0))
                                    .child(icon("git-branch", 16.0))
                                    .child(
                                        div()
                                            .min_w(px(0.0))
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .child(repo.current_branch.clone()),
                                    ),
                            ),
                    )
                    .child(
                        div().flex().flex_row().items_center().gap(px(4.0)).child(
                            div().text_size(px(15.0)).text_color(dirty_color).child(
                                repo.remote_statuses
                                    .iter()
                                    .filter_map(|s| s.counts)
                                    .find(|c| *c != (0, 0))
                                    .map(|(ahead, behind)| format!("↑ {ahead}  ↓ {behind}"))
                                    .unwrap_or_else(|| {
                                        if repo.is_dirty {
                                            "有修改".into()
                                        } else {
                                            "干净".into()
                                        }
                                    }),
                            ),
                        ),
                    );

                let group = super::workspace_controls::needs_attention(repo);
                let mut items = Vec::new();
                if last_group != Some(group) {
                    let collapsed = self.collapsed_repo_groups.contains(&group);
                    let id = format!("repo-group-{group}");
                    items.push(
                        self.track(
                            &id,
                            div()
                                .id(ElementId::Name(id.clone().into()))
                                .px(px(16.0))
                                .pt(px(16.0))
                                .pb(px(8.0))
                                .text_size(px(18.0))
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .focusable()
                                .tab_stop(true)
                                .keyboard_focus(|s| {
                                    s.border_1().border_color(rgb(theme::palette().accent))
                                })
                                .cursor_pointer()
                                .child(icon(
                                    if collapsed {
                                        "chevron-right"
                                    } else {
                                        "chevron-down"
                                    },
                                    16.0,
                                ))
                                .child(format!(
                                    "{} ({})",
                                    if group {
                                        "需要处理"
                                    } else {
                                        "其他仓库"
                                    },
                                    self.repos
                                        .iter()
                                        .filter(|r| super::workspace_controls::needs_attention(r)
                                            == group)
                                        .count()
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !this.collapsed_repo_groups.remove(&group) {
                                        this.collapsed_repo_groups.insert(group);
                                    }
                                    cx.notify();
                                })),
                        ),
                    );
                    last_group = Some(group);
                }
                if self.collapsed_repo_groups.contains(&group) {
                    return items;
                }
                items.push(self.track(&format!("repo-{i}"), item));
                if is_expanded {
                    items.extend(repo.submodules.iter().enumerate().map(|(j, submodule)| {
                        let id = format!("repo-{i}-submodule-{j}");
                        let is_selected = self.selected
                            == Some(RepoSelection::Submodule {
                                repo_index: i,
                                submodule_index: j,
                                relative_path: submodule.relative_path.clone(),
                            });
                        let bg = if is_selected {
                            rgb(theme::palette().accent_soft)
                        } else {
                            rgb(theme::palette().bg_surface)
                        };
                        let dirty_color = if submodule.is_dirty {
                            rgb(theme::palette().yellow)
                        } else if submodule.is_initialized {
                            rgb(theme::palette().green)
                        } else {
                            rgb(theme::palette().yellow)
                        };
                        let dirty_icon = if submodule.is_dirty {
                            "●"
                        } else if submodule.is_initialized {
                            "✓"
                        } else {
                            "!"
                        };
                        let path = submodule.path.clone();
                        let relative_path = submodule.relative_path.clone();
                        let submodule_detail = SubmoduleDetail {
                            name: submodule.name.clone(),
                            path: submodule.path.display().to_string(),
                            url: submodule.url.clone(),
                            is_initialized: submodule.is_initialized,
                        };
                        let is_initialized = submodule.is_initialized;

                        let item = div()
                            .id(ElementId::Name(id.clone().into()))
                            .tooltip(super::tooltip::text(format!(
                                "{}\n{}\n{}",
                                submodule.name,
                                submodule.path.display(),
                                submodule.current_branch
                            )))
                            .flex_shrink_0()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .pl(px(84.0))
                            .pr(px(12.0))
                            .py(px(6.0))
                            .bg(bg)
                            .border_l_2()
                            .border_color(if is_selected {
                                rgb(theme::palette().accent)
                            } else {
                                rgb(theme::palette().bg_surface)
                            })
                            .focusable()
                            .tab_stop(true)
                            .keyboard_focus(|s| {
                                s.border_1().border_color(rgb(theme::palette().accent))
                            })
                            .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(theme::palette().bg_elevated)))
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.begin_select_submodule(i, j, relative_path.clone());
                                cx.notify();
                                let path = path.clone();
                                let request = this.begin_detail_request(path.clone());
                                let submodule_detail = submodule_detail.clone();
                                this.detail_task = Some(cx.spawn(async move |entity, cx| {
                                    let (detail, log_entries, canvas_layout) = cx
                                        .background_executor()
                                        .spawn(async move {
                                            let (detail, log_entries) = if is_initialized {
                                                (
                                                    git_ops::get_repo_detail(&path),
                                                    git_ops::get_commit_log(&path, 200),
                                                )
                                            } else {
                                                (None, Vec::new())
                                            };
                                            (
                                                detail,
                                                log_entries,
                                                is_initialized
                                                    .then(|| {
                                                        commit_canvas::load_layout_for_path(
                                                            &path, 200,
                                                        )
                                                    })
                                                    .flatten(),
                                            )
                                        })
                                        .await;
                                    entity
                                        .update(cx, |this, cx| {
                                            this.apply_detail(
                                                request,
                                                detail,
                                                Some(submodule_detail),
                                                log_entries,
                                                canvas_layout,
                                            );
                                            this.refresh_visible_worktree(cx);
                                            cx.notify();
                                        })
                                        .ok();
                                }));
                            }))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_grow()
                                    .overflow_x_hidden()
                                    .child(
                                        div()
                                            .text_size(px(17.0))
                                            .flex()
                                            .items_center()
                                            .gap(px(10.0))
                                            .child(icon("folder", 20.0))
                                            .child(submodule.name.clone())
                                            .child(
                                                div()
                                                    .text_size(px(14.0))
                                                    .text_color(rgb(theme::palette().text_subtle))
                                                    .child("子模块"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(15.0))
                                            .text_color(rgb(theme::palette().text_subtle))
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .pl(px(30.0))
                                            .child(icon("git-branch", 16.0))
                                            .child(submodule.current_branch.clone()),
                                    ),
                            )
                            .child(
                                div().flex().flex_row().items_center().gap(px(4.0)).child(
                                    div().text_size(px(15.0)).text_color(dirty_color).child(
                                        submodule
                                            .remote_statuses
                                            .iter()
                                            .find(|s| s.counts.is_some_and(|c| c != (0, 0)))
                                            .map(|s| s.label())
                                            .unwrap_or_else(|| dirty_icon.into()),
                                    ),
                                ),
                            );

                        self.track(&id, item)
                    }));
                }

                items
            })
            .collect();

        let header = div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .px(px(18.0))
            .pt(px(16.0))
            .pb(px(12.0))
            .bg(rgb(theme::palette().bg_surface))
            .border_b_1()
            .border_color(rgb(theme::palette().border))
            .child(div().text_size(px(17.0)).child("工作区"))
            .children(
                self.repo_search
                    .clone()
                    .map(|input| self.track("repo-search", div().child(input))),
            )
            .child(self.render_repo_filters(cx));
        let list = div()
            .id("repo-list")
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .h_full()
            .bg(rgb(theme::palette().bg_surface))
            .border_r_1()
            .border_color(rgb(theme::palette().border))
            .overflow_y_scroll()
            .track_scroll(&self.repo_scroll)
            .children(if self.scanning && self.repos.is_empty() {
                vec![super::state_views::loading(
                    "正在扫描仓库…",
                    "仅扫描直接子目录",
                )]
            } else if repo_items.is_empty() && !self.scanning {
                vec![
                    div()
                        .p(px(14.0))
                        .text_size(px(15.0))
                        .child(if self.repos.is_empty() {
                            "当前工作区没有仓库"
                        } else {
                            "0 个结果 · 没有匹配的仓库"
                        })
                        .when(!self.repos.is_empty(), |empty| {
                            empty.child(
                                self.track(
                                    "repo-clear-filters",
                                    div()
                                        .id("repo-clear-filters")
                                        .py(px(10.0))
                                        .focusable()
                                        .tab_stop(true)
                                        .keyboard_focus(|s| {
                                            s.border_1().border_color(rgb(theme::palette().accent))
                                        })
                                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                        .cursor_pointer()
                                        .text_color(rgb(theme::palette().accent))
                                        .child("清除搜索与筛选")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.repo_filter =
                                                super::workspace_controls::RepoFilter::All;
                                            if let Some(input) = &this.repo_search {
                                                input.update(cx, |input, cx| input.clear(cx));
                                            }
                                            cx.notify();
                                        })),
                                ),
                            )
                        })
                        .into_any_element(),
                ]
            } else {
                repo_items
            });

        let handle = self.repo_scroll.clone();
        let entity = cx.entity();
        let scrollbar = canvas(
            |_, _, _| (),
            move |bounds, _, window, _cx| {
                let height: f32 = bounds.size.height.into();
                let max: f32 = handle.max_offset().height.into();
                let offset: f32 = handle.offset().y.into();
                let (thumb_height, thumb_top) = repo_scrollbar_geometry(height, max, offset);
                window.paint_quad(fill(bounds, rgb(theme::palette().bg_base)));
                let thumb = Bounds::new(
                    point(bounds.origin.x + px(2.0), bounds.origin.y + px(thumb_top)),
                    size(px(3.0), px(thumb_height)),
                );
                if max > 0.0 {
                    window.paint_quad(fill(thumb, rgb(theme::palette().bg_overlay)));
                }
                window.on_mouse_event({
                    let entity = entity.clone();
                    let handle = handle.clone();
                    move |event: &MouseDownEvent, phase, _, cx| {
                        if !phase.bubble()
                            || event.button != MouseButton::Left
                            || !bounds.contains(&event.position)
                        {
                            return;
                        }
                        let y: f32 = (event.position.y - bounds.origin.y).into();
                        let grab = if y >= thumb_top && y <= thumb_top + thumb_height {
                            y - thumb_top
                        } else {
                            thumb_height / 2.0
                        };
                        entity.update(cx, |this, cx| {
                            this.repo_scroll_drag = Some(grab);
                            cx.notify();
                        });
                        let fraction =
                            ((y - grab) / (height - thumb_height).max(1.0)).clamp(0.0, 1.0);
                        handle.set_offset(point(px(0.0), px(-max * fraction)));
                        cx.stop_propagation();
                    }
                });
                window.on_mouse_event({
                    let entity = entity.clone();
                    let handle = handle.clone();
                    move |event: &MouseMoveEvent, phase, _, cx| {
                        if !phase.bubble() {
                            return;
                        }
                        let Some(grab) = entity.read(cx).repo_scroll_drag else {
                            return;
                        };
                        if !event.dragging() {
                            return;
                        }
                        let y: f32 = (event.position.y - bounds.origin.y).into();
                        let fraction =
                            ((y - grab) / (height - thumb_height).max(1.0)).clamp(0.0, 1.0);
                        handle.set_offset(point(px(0.0), px(-max * fraction)));
                        entity.update(cx, |_, cx| cx.notify());
                        cx.stop_propagation();
                    }
                });
                window.on_mouse_event({
                    let entity = entity.clone();
                    move |event: &MouseUpEvent, _, _, cx| {
                        if event.button == MouseButton::Left
                            && entity.read(cx).repo_scroll_drag.is_some()
                        {
                            entity.update(cx, |this, cx| {
                                this.repo_scroll_drag = None;
                                cx.notify();
                            });
                        }
                    }
                });
            },
        )
        .w(px(4.0))
        .h_full()
        .flex_shrink_0();
        div()
            .flex()
            .flex_row()
            .w(px(if self.narrow_window { 260.0 } else { 356.0 }))
            .flex_shrink_0()
            .h_full()
            .min_h(px(0.0))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .child(header)
                    .child(list),
            )
            .child(scrollbar)
            .into_any_element()
    }

    pub fn render_context_menu(
        &self,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Option<AnyElement> {
        let menu = self.context_menu.as_ref()?;
        let repo_index = menu.repo_index;
        let position = menu.position;
        let target_path = menu
            .target_path
            .clone()
            .or_else(|| self.repos.get(repo_index).map(|r| r.path.clone()));
        let copied_path = target_path.clone();
        let items = [
            ("ctx-open-directory", "打开目录", false),
            ("ctx-open-terminal", "在此打开终端", true),
        ]
        .into_iter()
        .map(|(id, label, terminal)| {
            let target = target_path.clone();
            let button = div()
                .id(id)
                .px(px(12.0))
                .py(px(6.0))
                .focusable()
                .tab_stop(true)
                .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(theme::palette().bg_overlay)))
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(super::icons::icon(
                    if terminal { "terminal" } else { "folder" },
                    18.0,
                ))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.close_context_menu();
                    if let Some(path) = target.clone() {
                        this.open_overview_location(path, terminal, cx);
                    }
                }));
            self.track(id, button)
        });

        let menu_panel = div()
            .id("context-menu")
            // Block hit testing through this floating menu to repository rows.
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .w(px(200.0))
            .bg(rgb(theme::palette().bg_surface))
            .border_1()
            .border_color(rgb(theme::palette().border))
            .rounded(px(3.0))
            .py(px(4.0))
            .text_size(px(17.0))
            .text_color(rgb(theme::palette().text_primary))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.close_context_menu();
                cx.notify();
            }))
            .children(items)
            .child(
                self.track(
                    "ctx-copy-path",
                    div()
                        .id("ctx-copy-path")
                        .px(px(12.0))
                        .py(px(6.0))
                        .focusable()
                        .tab_stop(true)
                        .keyboard_focus(|s| s.border_1().border_color(rgb(theme::palette().accent)))
                        .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                        .cursor_pointer()
                        .border_t_1()
                        .border_color(rgb(theme::palette().border))
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(super::icons::icon("copy", 18.0))
                        .child("复制仓库路径")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(path) = &copied_path {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    path.display().to_string(),
                                ));
                            }
                            this.close_context_menu();
                            cx.notify();
                        })),
                ),
            );

        Some(deferred(anchored().position(position).child(menu_panel)).into_any_element())
    }

    pub(super) fn refresh_pushed_repo_details(
        &mut self,
        repo_index: usize,
        expected_path: &std::path::Path,
        cx: &mut Context<'_, Self>,
    ) {
        if self.selected != Some(RepoSelection::Repo(repo_index))
            || self.repos.get(repo_index).map(|repo| repo.path.as_path()) != Some(expected_path)
        {
            return;
        }
        self.reload_current_detail(cx);
    }
}
use gpui::prelude::FluentBuilder;
