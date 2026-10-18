use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{Value, json};

use super::view_tree::ViewNode;

pub type ViewTreeProvider = Arc<Mutex<Option<ViewNode>>>;

pub fn publish_tree(provider: &ViewTreeProvider, revision: u64, tree: ViewNode) {
    if let Ok(mut current) = provider.lock() {
        let previous = current
            .as_ref()
            .and_then(|node| node.state.as_ref())
            .and_then(|state| state.get("view_revision"))
            .and_then(Value::as_u64);
        // Cancellation cannot interrupt a snapshot build already running on a
        // worker. Compare under the publication lock so an older build cannot win.
        if previous.is_none_or(|previous| revision > previous) {
            *current = Some(tree);
        }
    }
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    use crate::app_state::GitMasterApp;

    #[test]
    fn late_and_duplicate_snapshots_cannot_replace_newer_completion() {
        let provider = ViewTreeProvider::default();
        let mut app = GitMasterApp::new();
        app.test_view_revision = 2;
        app.set_status("retry completed");
        publish_tree(&provider, 2, app.test_view_tree_snapshot().build());
        app.test_view_revision = 1;
        app.busy = true;
        app.set_status("older failure");
        publish_tree(&provider, 1, app.test_view_tree_snapshot().build());
        app.test_view_revision = 2;
        publish_tree(&provider, 2, app.test_view_tree_snapshot().build());
        let current = provider.lock().unwrap();
        let state = current.as_ref().unwrap().state.as_ref().unwrap();
        assert_eq!(state["view_revision"], 2);
        assert_eq!(state["busy"], false);
        assert_eq!(state["status_revision"], 1);
    }
}

#[derive(Debug)]
pub enum TestCommand {
    SelectRepo(usize),
    ToggleRepo(usize),
    SelectSubmodule {
        repo_index: usize,
        submodule_index: usize,
    },
    SetTab(String),
    SetLogView(String),
}

pub type CommandQueue = Arc<Mutex<Vec<TestCommand>>>;

pub fn start(
    tree_provider: ViewTreeProvider,
    logs_provider: ViewTreeProvider,
    command_queue: CommandQueue,
) {
    let port: u16 = std::env::var("GIT_MASTER_RPC_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9222);

    thread::spawn(move || {
        let addr = format!("127.0.0.1:{port}");
        let listener = match TcpListener::bind(&addr) {
            Ok(l) => {
                eprintln!("[test-rpc] listening on {addr}");
                l
            }
            Err(e) => {
                eprintln!("[test-rpc] failed to bind {addr}: {e}");
                return;
            }
        };

        for stream in listener.incoming() {
            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[test-rpc] accept error: {e}");
                    continue;
                }
            };
            let provider = tree_provider.clone();
            let cmds = command_queue.clone();
            let logs = logs_provider.clone();
            thread::spawn(move || {
                let reader = BufReader::new(match stream.try_clone() {
                    Ok(s) => s,
                    Err(_) => return,
                });
                let mut writer = stream;

                for line in reader.lines() {
                    let line = match line {
                        Ok(l) if !l.is_empty() => l,
                        Ok(_) => continue,
                        Err(_) => break,
                    };

                    if let Some(response) = handle_request(&line, &provider, &logs, &cmds) {
                        let mut out = serde_json::to_string(&response).unwrap_or_default();
                        out.push('\n');
                        if writer.write_all(out.as_bytes()).is_err() {
                            break;
                        }
                    }
                }
            });
        }
    });
}

fn handle_request(
    raw: &str,
    provider: &ViewTreeProvider,
    logs: &ViewTreeProvider,
    commands: &CommandQueue,
) -> Option<Value> {
    let req: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => {
            return Some(json!({
                "jsonrpc": "2.0",
                "error": { "code": -32700, "message": "Parse error" },
                "id": null
            }));
        }
    };

    req.get("id")?;

    let id = req["id"].clone();
    let method = req.get("method").and_then(|m| m.as_str());
    let params = req.get("params");

    match method {
        Some("get_view_tree") | Some("get_logs_view_tree") => {
            let selected = if method == Some("get_logs_view_tree") {
                logs
            } else {
                provider
            };
            let tree = selected.lock().ok().and_then(|g| g.clone());
            Some(json!({
                "jsonrpc": "2.0",
                "result": tree,
                "id": id
            }))
        }
        Some("select_repo") => {
            let index = params
                .and_then(|p| p.get("index"))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            match index {
                Some(i) => {
                    if let Ok(mut q) = commands.lock() {
                        q.push(TestCommand::SelectRepo(i));
                    }
                    Some(json!({ "jsonrpc": "2.0", "result": "ok", "id": id }))
                }
                None => Some(json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32602, "message": "Missing params.index" },
                    "id": id
                })),
            }
        }
        Some("toggle_repo") => {
            let index = params
                .and_then(|p| p.get("index"))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            match index {
                Some(i) => {
                    if let Ok(mut q) = commands.lock() {
                        q.push(TestCommand::ToggleRepo(i));
                    }
                    Some(json!({ "jsonrpc": "2.0", "result": "ok", "id": id }))
                }
                None => Some(json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32602, "message": "Missing params.index" },
                    "id": id
                })),
            }
        }
        Some("select_submodule") => {
            let repo_index = params
                .and_then(|p| p.get("repo_index"))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            let submodule_index = params
                .and_then(|p| p.get("submodule_index"))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            match (repo_index, submodule_index) {
                (Some(repo_index), Some(submodule_index)) => {
                    if let Ok(mut q) = commands.lock() {
                        q.push(TestCommand::SelectSubmodule {
                            repo_index,
                            submodule_index,
                        });
                    }
                    Some(json!({ "jsonrpc": "2.0", "result": "ok", "id": id }))
                }
                _ => Some(json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32602,
                        "message": "Missing params.repo_index or params.submodule_index"
                    },
                    "id": id
                })),
            }
        }
        Some("set_tab") => {
            let tab = params
                .and_then(|p| p.get("tab"))
                .and_then(|v| v.as_str())
                .map(String::from);
            match tab {
                Some(t) => {
                    if let Ok(mut q) = commands.lock() {
                        q.push(TestCommand::SetTab(t));
                    }
                    Some(json!({ "jsonrpc": "2.0", "result": "ok", "id": id }))
                }
                None => Some(json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32602, "message": "Missing params.tab" },
                    "id": id
                })),
            }
        }
        Some("set_log_view") => {
            let view = params
                .and_then(|p| p.get("view"))
                .and_then(|v| v.as_str())
                .map(String::from);
            match view {
                Some(view) => {
                    if let Ok(mut q) = commands.lock() {
                        q.push(TestCommand::SetLogView(view));
                    }
                    Some(json!({ "jsonrpc": "2.0", "result": "ok", "id": id }))
                }
                None => Some(json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32602, "message": "Missing params.view" },
                    "id": id
                })),
            }
        }
        Some(_) => Some(json!({
            "jsonrpc": "2.0",
            "error": { "code": -32601, "message": "Method not found" },
            "id": id
        })),
        None => Some(json!({
            "jsonrpc": "2.0",
            "error": { "code": -32600, "message": "Invalid Request" },
            "id": id
        })),
    }
}
