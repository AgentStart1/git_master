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
        let (tree_provider, command_queue) = {
            let provider = test_rpc::server::ViewTreeProvider::default();
            let cmds = test_rpc::server::CommandQueue::default();
            test_rpc::server::start(provider.clone(), cmds.clone());
            (provider, cmds)
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
                |_window, cx| {
                    cx.new(|_cx| {
                        #[allow(unused_mut)]
                        let mut app = GitMasterApp::new();
                        #[cfg(feature = "test-rpc")]
                        {
                            app.tree_provider = tree_provider.clone();
                            app.command_queue = command_queue.clone();
                        }
                        app
                    })
                },
            )
            .unwrap();

        #[cfg(feature = "test-rpc")]
        {
            if let Ok(entity) = window.entity(cx) {
                if let Ok(dir) = std::env::var("GIT_MASTER_OPEN_DIR") {
                    let path = std::path::PathBuf::from(&dir);
                    entity.update(cx, |this, cx| {
                        this.begin_scan(path.clone());
                        cx.notify();
                    });
                    let scan_path = path.clone();
                    let scan_entity = entity.downgrade();
                    let scan_task = cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                        let repos = cx
                            .background_executor()
                            .spawn(async move { git_ops::scan_repos(&scan_path) })
                            .await;
                        scan_entity
                            .update(cx, |this, cx| {
                                this.apply_scan(&path, repos);
                                cx.notify();
                            })
                            .ok();
                    });
                    entity.update(cx, |this, _cx| {
                        this.scan_task = Some(scan_task);
                    });
                }

                let cmds_watch = command_queue.clone();
                let entity_watch = entity.downgrade();
                cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                    loop {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(100))
                            .await;
                        let queue = cmds_watch.clone();
                        let commands = cx
                            .background_executor()
                            .spawn(async move {
                                queue
                                    .lock()
                                    .ok()
                                    .map(|mut commands| commands.drain(..).collect::<Vec<_>>())
                                    .unwrap_or_default()
                            })
                            .await;

                        for command in commands {
                            let Ok(request) = entity_watch.update(cx, |this, cx| {
                                let request = this.prepare_test_command(command);
                                cx.notify();
                                request
                            }) else {
                                return;
                            };

                            if let Some(request) = request {
                                let result = cx
                                    .background_executor()
                                    .spawn(async move { request.load() })
                                    .await;
                                if entity_watch
                                    .update(cx, |this, cx| {
                                        this.apply_test_detail(result);
                                        cx.notify();
                                    })
                                    .is_err()
                                {
                                    return;
                                }
                            }
                        }
                    }
                })
                .detach();
            }
        }
    });
}
