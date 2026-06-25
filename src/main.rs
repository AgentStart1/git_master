mod app_state;
mod git_ops;
mod models;
#[cfg(feature = "test-rpc")]
mod test_rpc;
mod ui;

use app_state::GitMasterApp;
use gpui::*;

fn main() {
    Application::new().run(|cx: &mut App| {
        #[cfg(feature = "test-rpc")]
        let tree_provider = {
            let provider = test_rpc::server::ViewTreeProvider::default();
            test_rpc::server::start(provider.clone());
            provider
        };

        #[allow(unused_variables)]
        let window = cx
            .open_window(
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

        #[cfg(feature = "test-rpc")]
        {
            if let Ok(entity) = window.entity(cx) {
                let entity_for_read = entity.clone();
                cx.observe(&entity, move |_, cx| {
                    let tree = entity_for_read.read(cx).build_view_tree();
                    if let Ok(mut guard) = tree_provider.lock() {
                        *guard = Some(tree);
                    }
                })
                .detach();
            }
        }
    });
}
