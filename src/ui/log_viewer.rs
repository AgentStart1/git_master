use super::focus_visibility::KeyboardFocusStyle;
use gpui::*;

use super::theme;
use crate::app_state::GitMasterApp;
use crate::log_viewer::{LogViewerHost, State, Viewer};

pub struct LogsWindow {
    viewer: Viewer,
    selected_row: Option<usize>,
    file_notice: Option<String>,
    buffer_banner_dismissed: bool,
    selected_file: Option<std::path::PathBuf>,
    scroll: ListState,
    rows: std::sync::Arc<[String]>,
    tail_bounds: super::log_geometry::RowBounds,
    input_revision: u64,
    frame_revision: u64,
    pending_end_check: Option<u64>,
    window_size: Option<Size<Pixels>>,
    reading: bool,
    manual_requested: bool,
    completed_reads: u64,
    poll_task: Option<Task<()>>,
    #[cfg(feature = "test-rpc")]
    poll_ticks: u64,
    #[cfg(feature = "test-rpc")]
    active_read_manual: bool,
    revision: u64,
    task: Option<Task<()>>,
    #[cfg(feature = "test-rpc")]
    bounds_registry: crate::test_rpc::tracked::BoundsRegistry,
    #[cfg(feature = "test-rpc")]
    tree_provider: crate::test_rpc::server::ViewTreeProvider,
    #[cfg(feature = "test-rpc")]
    view_revision: u64,
    #[cfg(feature = "test-rpc")]
    publication_owner: (WeakEntity<GitMasterApp>, u64),
}

impl GitMasterApp {
    pub(crate) fn open_logs(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(handle) = self.log_window {
            if handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return;
            }
        }
        self.log_window_generation += 1;
        let generation = self.log_window_generation;
        let parent = cx.entity().downgrade();
        #[cfg(feature = "test-rpc")]
        let provider = self.logs_tree_provider.clone();
        #[cfg(feature = "test-rpc")]
        if let Ok(mut tree) = provider.lock() {
            *tree = None;
        }
        let bounds = Bounds::centered(None, size(px(1440.0), px(1024.0)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Git Master · 操作日志".into()),
                    appears_transparent: cfg!(windows),
                    ..Default::default()
                }),
                focus: true,
                show: true,
                ..Default::default()
            },
            move |_, cx| {
                cx.new(|cx| {
                    #[cfg(feature = "test-rpc")]
                    let publication_owner = (parent.clone(), generation);
                    cx.on_release(move |_, cx| {
                        parent
                            .update(cx, |this, cx| {
                                if this.log_window_generation == generation {
                                    this.log_window = None;
                                    #[cfg(feature = "test-rpc")]
                                    if let Ok(mut tree) = this.logs_tree_provider.lock() {
                                        *tree = None;
                                    }
                                    cx.notify();
                                }
                            })
                            .ok();
                    })
                    .detach();
                    let mut view = LogsWindow {
                        viewer: Viewer::new(),
                        selected_row: None,
                        file_notice: None,
                        buffer_banner_dismissed: false,
                        selected_file: None,
                        scroll: ListState::new(1, ListAlignment::Top, px(100.0)),
                        rows: vec![String::new()].into(),
                        tail_bounds: Default::default(),
                        input_revision: 0,
                        frame_revision: 0,
                        pending_end_check: None,
                        window_size: None,
                        reading: false,
                        manual_requested: false,
                        completed_reads: 0,
                        poll_task: None,
                        #[cfg(feature = "test-rpc")]
                        poll_ticks: 0,
                        #[cfg(feature = "test-rpc")]
                        active_read_manual: false,
                        revision: 0,
                        task: None,
                        #[cfg(feature = "test-rpc")]
                        bounds_registry: Default::default(),
                        #[cfg(feature = "test-rpc")]
                        tree_provider: provider,
                        #[cfg(feature = "test-rpc")]
                        view_revision: 0,
                        #[cfg(feature = "test-rpc")]
                        publication_owner,
                    };
                    view.refresh(true, cx);
                    view.poll_task = Some(cx.spawn(async move |entity, cx| {
                        loop {
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(1))
                                .await;
                            if entity
                                .update(cx, |this, cx| {
                                    #[cfg(feature = "test-rpc")]
                                    {
                                        this.poll_ticks += 1;
                                        cx.notify();
                                    }
                                    this.refresh(false, cx);
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                    }));
                    view
                })
            },
        );
        match result {
            Ok(handle) => self.log_window = Some(handle),
            Err(error) => self.set_failure(format!("Cannot open logs: {error}")),
        }
        cx.notify();
    }
}

impl LogsWindow {
    fn preserve_wrapped_anchor(&self, previous_width: Pixels, window: &mut Window, cx: &mut App) {
        let top = self.scroll.logical_scroll_top();
        let Some(bounds) = self
            .scroll
            .bounds_for_item(top.item_ix)
            .filter(|bounds| bounds.size.height > px(0.0))
        else {
            return;
        };
        let Some(row) = self.rows.get(top.item_ix) else {
            return;
        };
        // GPUI invalidates widths but retains a pixel offset. Preserve the
        // fraction of the wrapped row, otherwise widening can skip it.
        let fraction = (top.offset_in_item / bounds.size.height).clamp(0.0, 1.0);
        let width = (self.scroll.viewport_bounds().size.width + window.viewport_size().width
            - previous_width)
            .max(px(1.0));
        let height = log_row(row.clone())
            .into_any_element()
            .layout_as_root(
                size(AvailableSpace::Definite(width), AvailableSpace::MinContent),
                window,
                cx,
            )
            .height;
        self.scroll.scroll_to(ListOffset {
            item_ix: top.item_ix,
            offset_in_item: height * fraction,
        });
    }

    fn refresh(&mut self, manual: bool, cx: &mut Context<'_, Self>) {
        if self.reading {
            self.manual_requested |= manual;
            return;
        }
        self.reading = true;
        #[cfg(feature = "test-rpc")]
        {
            self.active_read_manual = manual;
        }
        self.revision += 1;
        let revision = self.revision;

        let selected = self.selected_file.clone();
        self.task = Some(cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if selected.is_none() {
                        LogViewerHost::load()
                    } else {
                        LogViewerHost::load_file(selected)
                    }
                })
                .await;
            entity
                .update(cx, |this, cx| {
                    if this.revision != revision {
                        return;
                    }
                    this.reading = false;
                    this.completed_reads += 1;
                    if let (State::Ready(current), Ok(next)) = (&this.viewer.state, &result) {
                        if current.path != next.path {
                            this.file_notice = Some(if this.selected_file.is_none() {
                                "日期切换，已读取当前日志文件".into()
                            } else {
                                "已切换日志文件".into()
                            });
                        }
                    }
                    let previous = this.viewer.pending.clone();
                    this.viewer.accept(result);
                    if this.viewer.pending != previous {
                        this.buffer_banner_dismissed = false;
                    }
                    cx.notify();
                    if std::mem::take(&mut this.manual_requested) {
                        this.refresh(true, cx);
                    }
                })
                .ok();
        }));
    }

    fn choose_log_file(&mut self, cx: &mut Context<Self>) {
        if self.reading {
            return;
        }
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择日志文件".into()),
        });
        cx.spawn(async move |entity, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                if let Some(path) = paths.into_iter().next() {
                    entity
                        .update(cx, |this, cx| {
                            this.selected_file = Some(path);
                            this.viewer.resume();
                            this.refresh(true, cx);
                        })
                        .ok();
                }
            }
        })
        .detach();
    }
    fn track(&self, id: &str, element: impl IntoElement) -> AnyElement {
        #[cfg(feature = "test-rpc")]
        return crate::test_rpc::tracked::tracked(id, element, &self.bounds_registry)
            .into_any_element();
        #[cfg(not(feature = "test-rpc"))]
        {
            let _ = id;
            element.into_any_element()
        }
    }

    pub(crate) fn render_logs(&self, cx: &mut Context<'_, Self>) -> AnyElement {
        let state = &self.viewer.state;
        let copy = matches!(state, State::Ready(snapshot) if !snapshot.text.is_empty());
        let file = match state {
            State::Ready(s) => s
                .path
                .file_name()
                .unwrap_or(s.path.as_os_str())
                .to_string_lossy()
                .into_owned(),
            _ => "选择日志文件".into(),
        };
        let mut toolbar = div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap(px(12.0))
            .flex_shrink_0()
            .child(div().text_size(px(18.0)).child("日志文件"))
            .child(
                self.track(
                    "logs-select",
                    div()
                        .id("logs-select")
                        .flex_1()
                        .min_w(px(280.0))
                        .px(px(12.0))
                        .py(px(9.0))
                        .border_1()
                        .border_color(rgb(theme::palette().border))
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(super::icons::icon("file", 20.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(17.0))
                                .child(file),
                        )
                        .child(super::icons::icon("chevron-down", 18.0))
                        .focusable()
                        .tab_stop(true)
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| this.choose_log_file(cx))),
                ),
            );
        for (id, label, enabled) in [
            (
                "logs-follow",
                if self.viewer.following {
                    "暂停跟随"
                } else {
                    "继续跟随"
                },
                true,
            ),
            ("logs-today", "今日日志", !self.reading),
            ("logs-refresh", "刷新", true),
            ("logs-copy", "复制当前显示", copy),
        ] {
            toolbar = toolbar.child(
                self.track(
                    id,
                    div()
                        .id(id)
                        .flex_shrink_0()
                        .px(px(12.0))
                        .py(px(7.0))
                        .rounded(px(3.0))
                        .bg(rgb(theme::palette().bg_overlay))
                        .text_sm()
                        .text_color(rgb(if enabled {
                            theme::palette().text_primary
                        } else {
                            theme::palette().text_muted
                        }))
                        .cursor(if enabled {
                            CursorStyle::PointingHand
                        } else {
                            CursorStyle::Arrow
                        })
                        .child(label)
                        .on_click(cx.listener(move |this, _, window, cx| match id {
                            "logs-follow" => {
                                this.input_revision += 1;
                                this.pending_end_check = None;
                                if this.viewer.following {
                                    this.viewer.following = false;
                                } else {
                                    this.viewer.resume();
                                }
                                cx.notify();
                            }
                            "logs-today" if !this.reading => {
                                this.selected_file = None;
                                this.viewer.resume();
                                this.refresh(true, cx);
                            }
                            "logs-directory" => {
                                cx.background_executor()
                                    .spawn(async {
                                        if let Some(path) =
                                            crate::operation_log::log_path().parent()
                                        {
                                            let _ = crate::desktop_actions::open(path, false);
                                        }
                                    })
                                    .detach();
                            }
                            "logs-refresh" => this.refresh(true, cx),
                            "logs-close" => window.remove_window(),
                            _ => {
                                if let State::Ready(snapshot) = &this.viewer.state {
                                    if !snapshot.text.is_empty() {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            snapshot.text.clone(),
                                        ));
                                    }
                                }
                            }
                        })),
                ),
            );
        }
        let (notice, color) = match state {
            State::Loading => (None, theme::palette().text_subtle),
            State::Failed(_) => (None, theme::palette().red),
            State::Ready(snapshot) => (
                snapshot
                    .truncated
                    .then_some("已显示有界尾部：最近 2000 行 / 256 KiB · 完整文件保留在磁盘"),
                theme::palette().text_subtle,
            ),
        };
        let rows = self.rows.clone();
        let tail_bounds = self.tail_bounds.clone();
        #[cfg(feature = "test-rpc")]
        let registry = self.bounds_registry.clone();
        let owner = cx.entity().downgrade();
        let selected = self.selected_row;
        let content = list(self.scroll.clone(), move |index, _, _| {
            let text = rows[index].clone();
            let copied = text.clone();
            let row_owner = owner.clone();
            let row = div()
                .id(ElementId::Name(format!("log-line-select-{index}").into()))
                .relative()
                .bg(rgb(if selected == Some(index) {
                    theme::palette().accent_soft
                } else {
                    theme::palette().bg_base
                }))
                .child(log_row(text))
                .on_click(move |_, _, cx| {
                    row_owner
                        .update(cx, |this, cx| {
                            this.selected_row = Some(index);
                            cx.notify();
                        })
                        .ok();
                })
                .children((selected == Some(index)).then(|| {
                    div()
                        .id(ElementId::Name(format!("log-line-copy-{index}").into()))
                        .absolute()
                        .right(px(4.0))
                        .top_0()
                        .px(px(5.0))
                        .cursor_pointer()
                        .child(super::icons::icon("copy", 18.0))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()));
                            cx.stop_propagation();
                        })
                }));
            let row = if index + 1 == rows.len() {
                super::log_geometry::row_bounds(row, tail_bounds.clone()).into_any_element()
            } else {
                row.into_any_element()
            };
            #[cfg(feature = "test-rpc")]
            {
                let row =
                    crate::test_rpc::tracked::tracked(&format!("logs-row-{index}"), row, &registry);
                if index + 1 == rows.len() {
                    return crate::test_rpc::tracked::tracked("logs-end", row, &registry)
                        .into_any_element();
                }
                row.into_any_element()
            }
            #[cfg(not(feature = "test-rpc"))]
            row.into_any_element()
        })
        .size_full();
        self.track(
            "logs-panel",
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .p(px(16.0))
                .gap(px(12.0))
                .child(toolbar)
                .child(
                    div()
                        .text_size(px(15.0))
                        .text_color(rgb(theme::palette().text_subtle))
                        .child(format!(
                            "当前显示 {} 行 · 最多保留最近 2000 行 / 256 KiB · 完整文件保留在磁盘",
                            match state {
                                State::Ready(s) => s.text.lines().count(),
                                _ => 0,
                            }
                        )),
                )
                .child({
                    let buffered = !self.viewer.following
                        && self.viewer.pending.is_some()
                        && !self.buffer_banner_dismissed;
                    let text = if let (State::Ready(current), Some(pending)) =
                        (state, self.viewer.pending.as_ref())
                    {
                        pending
                            .text
                            .strip_prefix(&current.text)
                            .map(|tail| {
                                format!("有 {} 行新日志 · 继续跟随最新", tail.lines().count())
                            })
                            .unwrap_or_else(|| "日志尾部已更新 · 继续跟随最新".into())
                    } else if self.viewer.following {
                        self.file_notice
                            .clone()
                            .unwrap_or_else(|| "正在跟随最新日志".into())
                    } else {
                        "跟随已暂停 · 新日志将继续缓冲".into()
                    };
                    self.track(
                        "logs-buffered-banner",
                        div()
                            .id("logs-buffered-banner")
                            .h(px(48.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .px(px(12.0))
                            .border_1()
                            .border_color(rgb(if buffered {
                                theme::palette().yellow
                            } else {
                                theme::palette().border
                            }))
                            .bg(rgb(theme::palette().bg_elevated))
                            .text_size(px(16.0))
                            .text_color(rgb(if buffered {
                                theme::palette().yellow
                            } else {
                                theme::palette().text_subtle
                            }))
                            .child(super::icons::icon(
                                if self.viewer.following {
                                    "player-play"
                                } else {
                                    "player-pause"
                                },
                                20.0,
                            ))
                            .child(div().flex_1().min_w_0().child(
                                if self.buffer_banner_dismissed {
                                    "跟随已暂停 · 新日志已缓冲".to_owned()
                                } else {
                                    text
                                },
                            ))
                            .child(
                                self.track(
                                    "logs-banner-dismiss",
                                    div()
                                        .id("logs-banner-dismiss")
                                        .cursor_pointer()
                                        .child(super::icons::icon("x", 18.0))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.buffer_banner_dismissed = true;
                                            cx.stop_propagation();
                                            cx.notify();
                                        })),
                                ),
                            )
                            .focusable()
                            .tab_stop(true)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.viewer.resume();
                                cx.notify();
                            })),
                    )
                })
                .child(
                    self.track(
                        if self.viewer.refresh_error.is_some() {
                            "logs-refresh-error"
                        } else {
                            "logs-live-status"
                        },
                        div()
                            .h(px(20.0))
                            .flex_shrink_0()
                            .text_xs()
                            .text_ellipsis()
                            .text_color(rgb(if self.viewer.refresh_error.is_some() {
                                theme::palette().red
                            } else if self.viewer.following {
                                theme::palette().green
                            } else {
                                theme::palette().yellow
                            }))
                            .child(if let Some(error) = &self.viewer.refresh_error {
                                format!(
                                    "读取失败，正在自动重试 · {} · {error}",
                                    if self.viewer.following {
                                        "正在跟随最新日志"
                                    } else {
                                        "已暂停跟随"
                                    }
                                )
                            } else if self.viewer.following {
                                "每秒自动读取 · 正在跟随最新日志".into()
                            } else if self.viewer.pending.is_some() {
                                "每秒自动读取 · 已暂停跟随 · 有新日志缓冲，点击继续跟随查看".into()
                            } else {
                                "每秒自动读取 · 已暂停跟随".into()
                            }),
                    ),
                )
                .children(notice.map(|message| {
                    div()
                        .text_xs()
                        .text_color(rgb(theme::palette().yellow))
                        .child(message)
                }))
                .child(
                    self.track(
                        "logs-content",
                        div()
                            .id("logs-content")
                            .flex_1()
                            .min_h(px(0.0))
                            .overflow_hidden()
                            .on_scroll_wheel(cx.listener(
                                |this, event: &ScrollWheelEvent, _, cx| {
                                    this.input_revision += 1;
                                    let delta = event.delta.pixel_delta(px(16.0)).y;
                                    if delta > px(0.0) {
                                        this.pending_end_check = None;
                                        this.viewer.following = false;
                                        cx.notify();
                                    } else if delta < px(0.0) {
                                        // Validate the actual final row after this input is painted.
                                        this.pending_end_check = Some(this.input_revision);
                                        cx.notify();
                                    }
                                },
                            ))
                            .flex()
                            .flex_col()
                            .p(px(14.0))
                            .rounded(px(3.0))
                            .border_1()
                            .border_color(rgb(theme::palette().border))
                            .bg(rgb(theme::palette().bg_base))
                            .text_color(rgb(color))
                            .child(content),
                    ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .h(px(40.0))
                        .border_t_1()
                        .border_color(rgb(theme::palette().border))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(15.0))
                                .child(if self.viewer.following {
                                    "自动刷新每 1 秒 · 正在跟随最新日志"
                                } else {
                                    "自动刷新每 1 秒 · 已暂停跟随，仍在缓冲新日志"
                                }),
                        )
                        .child(
                            self.track(
                                "logs-bottom",
                                div()
                                    .id("logs-bottom")
                                    .px(px(12.0))
                                    .focusable()
                                    .tab_stop(true)
                                    .cursor_pointer()
                                    .child("滚动到底部 / 继续跟随")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.viewer.resume();
                                        cx.notify();
                                    })),
                            ),
                        )
                        .child(
                            self.track(
                                "logs-directory",
                                div()
                                    .id("logs-directory")
                                    .focusable()
                                    .tab_stop(true)
                                    .keyboard_focus(|s| {
                                        s.border_1().border_color(rgb(theme::palette().accent))
                                    })
                                    .active(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                    .cursor_pointer()
                                    .text_color(rgb(theme::palette().accent))
                                    .child("打开日志目录")
                                    .on_click(|_, _, cx| {
                                        cx.background_executor()
                                            .spawn(async {
                                                if let Some(path) =
                                                    crate::operation_log::log_path().parent()
                                                {
                                                    let _ =
                                                        crate::desktop_actions::open(path, false);
                                                }
                                            })
                                            .detach();
                                    }),
                            ),
                        ),
                ),
        )
        .into_any_element()
    }
}

impl Render for LogsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        #[cfg(feature = "test-rpc")]
        self.bounds_registry.borrow_mut().clear();
        let rows = match &self.viewer.state {
            State::Ready(snapshot) if !snapshot.text.is_empty() => snapshot.rows.clone(),
            state if self.rows.len() == 1 && self.rows[0] == state.display_text() => {
                self.rows.clone()
            }
            state => vec![state.display_text().to_owned()].into(),
        };
        if !std::sync::Arc::ptr_eq(&rows, &self.rows) {
            self.input_revision += 1;
            self.pending_end_check = None;
            self.scroll.reset(rows.len());
            self.rows = rows;
        } else if !self.viewer.following {
            if let Some(previous) = self
                .window_size
                .filter(|previous| previous.width != window.viewport_size().width)
            {
                self.preserve_wrapped_anchor(previous.width, window, cx);
            }
        }
        let window_size = window.viewport_size();
        if self
            .window_size
            .is_some_and(|previous| previous != window_size)
        {
            self.input_revision += 1;
            self.pending_end_check = None;
        }
        self.window_size = Some(window_size);
        if self.viewer.following {
            self.scroll.scroll_to(ListOffset {
                item_ix: self.rows.len(),
                offset_in_item: px(0.0),
            });
        }
        let panel = div()
            .on_key_down(|event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "tab" {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev();
                    } else {
                        window.focus_next();
                    }
                    cx.stop_propagation();
                }
            })
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::palette().bg_surface))
            .text_color(rgb(theme::palette().text_primary))
            .font_family("Segoe UI")
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(48.0))
                    .px(px(20.0))
                    .gap(px(14.0))
                    .border_b_1()
                    .border_color(rgb(theme::palette().border))
                    .child(super::icons::icon("git-branch", 24.0))
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .items_center()
                            .window_control_area(WindowControlArea::Drag)
                            .text_size(px(21.0))
                            .child("Git Master · 操作日志"),
                    )
                    .children(
                        [
                            (theme::ThemeMode::Light, "浅色"),
                            (theme::ThemeMode::Dark, "深色"),
                        ]
                        .into_iter()
                        .map(|(mode, label)| {
                            let id = format!("logs-theme-{}", mode.name());
                            self.track(
                                &id,
                                div()
                                    .id(ElementId::Name(id.clone().into()))
                                    .p(px(8.0))
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
                                    .child(label)
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        theme::set_mode(mode);
                                        let _ = crate::preferences::save_current();
                                        cx.refresh_windows();
                                    })),
                            )
                        }),
                    )
                    .children(
                        [
                            ("logs-minimize", "minus", WindowControlArea::Min),
                            ("logs-maximize", "square", WindowControlArea::Max),
                        ]
                        .into_iter()
                        .map(|(id, symbol, area)| {
                            div()
                                .id(id)
                                .w(px(44.0))
                                .h_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .hover(|s| s.bg(rgb(theme::palette().bg_overlay)))
                                .child(super::icons::icon(symbol, 20.0))
                                .window_control_area(area)
                        }),
                    )
                    .child(
                        self.track(
                            "logs-close",
                            div()
                                .id("logs-close")
                                .w(px(44.0))
                                .h_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .hover(|s| s.bg(rgb(theme::palette().red)))
                                .child(super::icons::icon("x", 22.0))
                                .on_click(|_, window, _| window.remove_window()),
                        ),
                    ),
            )
            .child(self.render_logs(cx));
        self.frame_revision += 1;
        let frame_revision = self.frame_revision;
        let input_revision = self.input_revision;
        self.tail_bounds.set(None);
        let entity = cx.entity();
        #[cfg(feature = "test-rpc")]
        let painted_viewer = self.viewer.clone();
        #[cfg(feature = "test-rpc")]
        let observation = crate::test_rpc::logs::Observation {
            viewport: Default::default(),
            completed_reads: self.completed_reads,
            generation: self.publication_owner.1,
            poll_ticks: self.poll_ticks,
            active_read: self.reading.then_some(self.revision),
            active_read_manual: self.reading.then_some(self.active_read_manual),
            manual_queued: self.manual_requested,
        };
        let panel = self.track("logs-root", super::focus_visibility::track_input(panel));
        super::log_geometry::after_paint(panel, move |cx| {
            entity.update(cx, |this, cx| {
                if this.frame_revision != frame_revision {
                    return;
                }
                #[cfg(feature = "test-rpc")]
                {
                    if this
                        .publication_owner
                        .0
                        .read_with(cx, |parent, _| {
                            parent.log_window_generation != this.publication_owner.1
                        })
                        .unwrap_or(false)
                    {
                        return;
                    }
                    this.view_revision += 1;
                    let tree = crate::test_rpc::logs::build_tree(
                        &painted_viewer,
                        &this.bounds_registry.borrow(),
                        this.view_revision,
                        crate::test_rpc::logs::Observation {
                            viewport: this.scroll.viewport_bounds(),
                            ..observation
                        },
                    );
                    crate::test_rpc::server::publish_tree(
                        &this.tree_provider,
                        this.view_revision,
                        tree,
                    );
                }
                if this.pending_end_check == Some(input_revision)
                    && this.input_revision == input_revision
                {
                    this.pending_end_check = None;
                    let viewport = this.scroll.viewport_bounds();
                    if this.tail_bounds.get().is_some_and(|tail| {
                        tail.bottom() >= viewport.top() + px(26.0)
                            && tail.bottom() <= viewport.bottom() + px(2.0)
                    }) {
                        this.viewer.resume();
                        cx.notify();
                    }
                }
            });
        })
        .into_any_element()
    }
}

fn log_row(text: String) -> Div {
    let mut highlights = Vec::new();
    if let Some((timestamp, _)) = text.split_once(' ')
        && chrono::DateTime::parse_from_rfc3339(timestamp).is_ok()
    {
        highlights.push((
            0..timestamp.len(),
            HighlightStyle {
                color: Some(rgb(theme::palette().accent).into()),
                ..Default::default()
            },
        ));
    }
    let error = text.contains("success=false")
        || text.contains("fatal:")
        || text.contains("ERROR")
        || text.contains("exit=exit code: 128");
    div()
        .text_color(rgb(if error {
            theme::palette().red
        } else {
            theme::palette().text_primary
        }))
        .min_h(px(26.0))
        .flex_shrink_0()
        .font_family(if cfg!(windows) {
            "Consolas"
        } else if cfg!(target_os = "macos") {
            "Menlo"
        } else {
            "monospace"
        })
        .text_size(px(16.0))
        .line_height(px(26.0))
        .child(StyledText::new(text).with_highlights(highlights))
}
