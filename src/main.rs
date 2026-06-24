mod app_state;
mod git_ops;
mod models;
mod ui;

use app_state::GitMasterApp;
use gpui::*;

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1200.0), px(800.0)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Git Master".into()),
                    ..Default::default()
                }),
                focus: true,
                show: true,
                ..Default::default()
            },
            |_window, cx| cx.new(|_cx| GitMasterApp::new()),
        )
        .unwrap();
    });
}
