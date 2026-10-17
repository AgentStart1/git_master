use gpui::*;

use super::theme;
use crate::app_state::GitMasterApp;
use crate::log_viewer::{LogViewerHost, State};

pub struct LogsWindow {
    state: State,
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
                        state: State::Loading,
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
                    view.refresh(cx);
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
    fn refresh(&mut self, cx: &mut Context<'_, Self>) {
        self.revision += 1;
        let revision = self.revision;
        self.state = State::Loading;
        cx.notify();
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
                    this.state = match result {
                        Ok(snapshot) => State::Ready(snapshot),
                        Err(error) => State::Failed(error),
                    };
                    cx.notify();
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
        let state = &self.state;
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
                            "logs-refresh" => this.refresh(cx),
                            "logs-close" => window.remove_window(),
                            _ => {
                                if let State::Ready(snapshot) = &this.state {
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
        let text = state.display_text();
        let (notice, color) = match state {
            State::Loading => (None, theme::TEXT_SUBTLE),
            State::Failed(_) => (None, theme::RED),
            State::Ready(snapshot) => (snapshot.truncated.then_some(
                "Showing the latest 2,000 lines, up to 256 KiB. Older entries remain in the log file."), theme::TEXT_SUBTLE),
        };
        let (body, last) = text.rsplit_once('\n').unwrap_or(("", text));
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
                            .overflow_y_scroll()
                            .overflow_x_scroll()
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
                            .text_color(rgb(color))
                            .child(div().flex_shrink_0().child(body.to_owned()))
                            .child(
                                self.track(
                                    "logs-end",
                                    div().flex_shrink_0().child(last.to_owned()),
                                ),
                            ),
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
                        let tree = crate::test_rpc::view_tree::build_logs_tree(
                            &this.state,
                            &this.bounds_registry.borrow(),
                            this.view_revision,
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
