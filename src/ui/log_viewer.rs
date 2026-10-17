use gpui::*;

use super::theme;
use crate::app_state::GitMasterApp;
use crate::log_viewer::{LogViewerHost, State, Viewer};

pub struct LogsWindow {
    viewer: Viewer,
    scroll: ListState,
    rows: std::sync::Arc<[String]>,
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
        let bounds = Bounds::centered(None, size(px(1000.0), px(700.0)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Git Master Logs".into()),
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
                        scroll: ListState::new(1, ListAlignment::Top, px(100.0)),
                        rows: vec![String::new()].into(),
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
            Err(error) => self.set_status(format!("Cannot open logs: {error}")),
        }
        cx.notify();
    }
}

impl LogsWindow {
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

        self.task = Some(cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async { LogViewerHost::load() })
                .await;
            entity
                .update(cx, |this, cx| {
                    if this.revision != revision {
                        return;
                    }
                    this.reading = false;
                    this.completed_reads += 1;
                    this.viewer.accept(result);
                    cx.notify();
                    if std::mem::take(&mut this.manual_requested) {
                        this.refresh(true, cx);
                    }
                })
                .ok();
        }));
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
        let mut toolbar = div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .flex_shrink_0()
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Application Logs"),
                    )
                    .children(match state {
                        State::Ready(snapshot) => Some(
                            div()
                                .text_xs()
                                .text_color(rgb(theme::TEXT_SUBTLE))
                                .text_ellipsis()
                                .child(snapshot.path.display().to_string()),
                        ),
                        _ => None,
                    }),
            );
        for (id, label, enabled) in [
            (
                "logs-follow",
                if self.viewer.following {
                    "Pause follow"
                } else {
                    "Follow latest"
                },
                true,
            ),
            ("logs-refresh", "Refresh", true),
            ("logs-copy", "Copy displayed", copy),
            ("logs-close", "Close", true),
        ] {
            toolbar = toolbar.child(
                self.track(
                    id,
                    div()
                        .id(id)
                        .flex_shrink_0()
                        .px(px(12.0))
                        .py(px(7.0))
                        .rounded(px(7.0))
                        .bg(rgb(theme::BG_OVERLAY))
                        .text_sm()
                        .text_color(rgb(if enabled {
                            theme::TEXT_PRIMARY
                        } else {
                            theme::TEXT_MUTED
                        }))
                        .cursor(if enabled {
                            CursorStyle::PointingHand
                        } else {
                            CursorStyle::Arrow
                        })
                        .child(label)
                        .on_click(cx.listener(move |this, _, window, cx| match id {
                            "logs-follow" => {
                                if this.viewer.following {
                                    this.viewer.following = false;
                                } else {
                                    this.viewer.resume();
                                }
                                cx.notify();
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
            State::Loading => (None, theme::TEXT_SUBTLE),
            State::Failed(_) => (None, theme::RED),
            State::Ready(snapshot) => (snapshot.truncated.then_some(
                "Showing the latest 2,000 lines, up to 256 KiB. Older entries remain in the log file."), theme::TEXT_SUBTLE),
        };
        let rows = self.rows.clone();
        #[cfg(feature = "test-rpc")]
        let registry = self.bounds_registry.clone();
        let content = list(self.scroll.clone(), move |index, _, _| {
            let row = div()
                .min_h(px(19.2))
                .flex_shrink_0()
                .child(rows[index].clone());
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
                .p(px(20.0))
                .gap(px(12.0))
                .child(toolbar)
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
                                theme::RED
                            } else if self.viewer.following {
                                theme::GREEN
                            } else {
                                theme::YELLOW
                            }))
                            .child(if let Some(error) = &self.viewer.refresh_error {
                                format!(
                                    "Auto refresh retrying · {} · {error}",
                                    if self.viewer.following {
                                        "Following latest"
                                    } else {
                                        "Follow paused"
                                    }
                                )
                            } else if self.viewer.following {
                                "Auto refresh: 1s · Following latest".into()
                            } else if self.viewer.pending.is_some() {
                                "Auto refresh: 1s · Follow paused · New logs available".into()
                            } else {
                                "Auto refresh: 1s · Follow paused".into()
                            }),
                    ),
                )
                .children(notice.map(|message| {
                    div()
                        .text_xs()
                        .text_color(rgb(theme::YELLOW))
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
                                |this, event: &ScrollWheelEvent, window, cx| {
                                    let delta = event.delta.pixel_delta(px(16.0)).y;
                                    if delta > px(0.0) {
                                        this.viewer.following = false;
                                        cx.notify();
                                    } else if delta < px(0.0) {
                                        // Read the handle after GPUI's default wheel listener moves it.
                                        cx.defer_in(window, |this, _, cx| {
                                            if this.scroll.max_offset_for_scrollbar().height
                                                + this.scroll.scroll_px_offset_for_scrollbar().y
                                                <= px(2.0)
                                            {
                                                this.viewer.resume();
                                                cx.notify();
                                            }
                                        });
                                    }
                                },
                            ))
                            .flex()
                            .flex_col()
                            .p(px(14.0))
                            .rounded(px(10.0))
                            .border_1()
                            .border_color(rgb(theme::BORDER))
                            .bg(rgb(theme::BG_BASE))
                            .font_family(if cfg!(windows) {
                                "Consolas"
                            } else if cfg!(target_os = "macos") {
                                "Menlo"
                            } else {
                                "monospace"
                            })
                            .text_xs()
                            .line_height(px(19.2))
                            .text_color(rgb(color))
                            .child(content),
                    ),
                ),
        )
        .into_any_element()
    }
}

impl Render for LogsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
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
            self.scroll.reset(rows.len());
            self.rows = rows;
        }
        if self.viewer.following {
            self.scroll.scroll_to(ListOffset {
                item_ix: self.rows.len(),
                offset_in_item: px(0.0),
            });
        }
        let panel = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG_SURFACE))
            .text_color(rgb(theme::TEXT_PRIMARY))
            .child(self.render_logs(cx));
        #[cfg(feature = "test-rpc")]
        {
            let entity = cx.entity();
            crate::test_rpc::tracked::tracked("logs-root", panel, &self.bounds_registry)
                .after_paint(move |cx| {
                    entity.update(cx, |this, cx| {
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
                            &this.viewer,
                            &this.bounds_registry.borrow(),
                            this.view_revision,
                            crate::test_rpc::logs::Observation {
                                completed_reads: this.completed_reads,
                                generation: this.publication_owner.1,
                                poll_ticks: this.poll_ticks,
                                active_read: this.reading.then_some(this.revision),
                                active_read_manual: this.reading.then_some(this.active_read_manual),
                                manual_queued: this.manual_requested,
                            },
                        );
                        crate::test_rpc::server::publish_tree(
                            &this.tree_provider,
                            this.view_revision,
                            tree,
                        );
                    });
                })
                .into_any_element()
        }
        #[cfg(not(feature = "test-rpc"))]
        panel.into_any_element()
    }
}
