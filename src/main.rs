mod app_state;
mod batch_ops;
mod commit_details;
mod desktop_actions;
mod diff_model;
mod diff_preview;
mod git_ops;
mod log_viewer;
mod models;
mod operation_log;
mod repo_actions;
#[cfg(feature = "test-rpc")]
mod test_rpc;
mod ui;
mod worktree;

use app_state::GitMasterApp;
use gpui::*;

fn main() {
    #[cfg(feature = "test-rpc")]
    if let Some(directory) = std::env::var_os("GIT_MASTER_TEST_CONFIG_DIR") {
        for level in [
            git2::ConfigLevel::System,
            git2::ConfigLevel::Global,
            git2::ConfigLevel::XDG,
            git2::ConfigLevel::ProgramData,
        ] {
            // libgit2 does not honor Git CLI's GIT_CONFIG_GLOBAL override.
            // Configure its process-wide paths before starting any UI or worker threads.
            unsafe { git2::opts::set_search_path(level, std::path::Path::new(&directory)) }
                .expect("Cannot isolate test Git configuration");
        }
    }
    if let Err(error) = operation_log::append("Application started") {
        eprintln!("Cannot initialize operation log: {error}");
    }
    Application::new().run(|cx: &mut App| {
        #[cfg(feature = "test-rpc")]
        let (tree_provider, logs_provider, command_queue) = {
            let provider = test_rpc::server::ViewTreeProvider::default();
            let cmds = test_rpc::server::CommandQueue::default();
            let logs = test_rpc::server::ViewTreeProvider::default();
            test_rpc::server::start(provider.clone(), logs.clone(), cmds.clone());
            (provider, logs, cmds)
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
                            app.logs_tree_provider = logs_provider.clone();
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
                if std::env::var("GIT_MASTER_TEST_PROMPTS").as_deref() == Ok("1") {
                    let bounds = entity.read(cx).bounds_registry.clone();
                    let prompts = entity.read(cx).test_prompt.clone();
                    cx.set_prompt_builder(move |_, message, detail, answers, handle, window, cx| {
                        test_rpc::prompt::build(message, detail, answers, handle, window, cx, bounds.clone(), prompts.clone())
                    });
                }
                if let Ok(dir) = std::env::var("GIT_MASTER_OPEN_DIR") {
                    let path = std::path::PathBuf::from(&dir);
                    entity.update(cx, |this, cx| {
                        this.start_scan(path, cx);
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
                                let refresh_worktree = matches!(&command, test_rpc::server::TestCommand::SetTab(tab) if tab == "change" || tab == "stash");
                                let request = this.prepare_test_command(command);
                                if refresh_worktree { this.run_worktree(worktree::Action::Refresh, cx); }
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
