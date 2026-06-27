use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{json, Value};

use super::view_tree::ViewNode;

pub type ViewTreeProvider = Arc<Mutex<Option<ViewNode>>>;

#[derive(Debug)]
pub enum TestCommand {
    SelectRepo(usize),
    SetTab(String),
}

pub type CommandQueue = Arc<Mutex<Vec<TestCommand>>>;

pub fn start(tree_provider: ViewTreeProvider, command_queue: CommandQueue) {
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

                    if let Some(response) = handle_request(&line, &provider, &cmds) {
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

    if req.get("id").is_none() {
        return None;
    }

    let id = req["id"].clone();
    let method = req.get("method").and_then(|m| m.as_str());
    let params = req.get("params");

    match method {
        Some("get_view_tree") => {
            let tree = provider.lock().ok().and_then(|g| g.clone());
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
