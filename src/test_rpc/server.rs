use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{json, Value};

use super::view_tree::ViewNode;

pub type ViewTreeProvider = Arc<Mutex<Option<ViewNode>>>;

pub fn start(tree_provider: ViewTreeProvider) {
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

        for stream in listener.incoming().flatten() {
            let provider = tree_provider.clone();
            thread::spawn(move || {
                let reader = BufReader::new(match stream.try_clone() {
                    Ok(s) => s,
                    Err(_) => return,
                });
                let mut writer = stream;

                for line in reader.lines() {
                    let line = match line {
                        Ok(l) if !l.is_empty() => l,
                        _ => break,
                    };

                    let response = handle_request(&line, &provider);
                    let mut out = serde_json::to_string(&response).unwrap_or_default();
                    out.push('\n');
                    if writer.write_all(out.as_bytes()).is_err() {
                        break;
                    }
                }
            });
        }
    });
}

fn handle_request(raw: &str, provider: &ViewTreeProvider) -> Value {
    let req: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => {
            return json!({
                "jsonrpc": "2.0",
                "error": { "code": -32700, "message": "Parse error" },
                "id": null
            });
        }
    };

    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

    match method {
        "get_view_tree" => {
            let tree = provider.lock().ok().and_then(|g| g.clone());
            match tree {
                Some(t) => json!({
                    "jsonrpc": "2.0",
                    "result": t,
                    "id": id
                }),
                None => json!({
                    "jsonrpc": "2.0",
                    "result": null,
                    "id": id
                }),
            }
        }
        _ => json!({
            "jsonrpc": "2.0",
            "error": { "code": -32601, "message": "Method not found" },
            "id": id
        }),
    }
}
