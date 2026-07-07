#!/usr/bin/env bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
RPC_PORT="${RPC_PORT:-9222}"
APP_PID=""

cleanup_e2e() {
    if [ -n "$APP_PID" ]; then
        kill "$APP_PID" 2>/dev/null
        wait "$APP_PID" 2>/dev/null || true
    fi
    [ -n "${TEST_DIR:-}" ] && rm -rf "$TEST_DIR"
}

fail() { echo "FAIL: $1"; exit 1; }
pass() { echo "PASS: $1"; }

RPC_ID=0
rpc() {
    RPC_ID=$((RPC_ID + 1))
    echo "$1" | nc -q 1 127.0.0.1 "$RPC_PORT" 2>/dev/null
}

wait_for_rpc() {
    for _ in $(seq 1 30); do
        if rpc '{"jsonrpc":"2.0","method":"get_view_tree","id":0}' | grep -q '"result"' 2>/dev/null; then
            return 0
        fi
        sleep 0.5
    done
    fail "RPC server did not become ready"
}

get_tree() {
    RPC_ID=$((RPC_ID + 1))
    rpc "{\"jsonrpc\":\"2.0\",\"method\":\"get_view_tree\",\"id\":$RPC_ID}"
}

rpc_select_repo() {
    RPC_ID=$((RPC_ID + 1))
    rpc "{\"jsonrpc\":\"2.0\",\"method\":\"select_repo\",\"params\":{\"index\":$1},\"id\":$RPC_ID}"
}

rpc_toggle_repo() {
    RPC_ID=$((RPC_ID + 1))
    rpc "{\"jsonrpc\":\"2.0\",\"method\":\"toggle_repo\",\"params\":{\"index\":$1},\"id\":$RPC_ID}"
}

rpc_select_submodule() {
    RPC_ID=$((RPC_ID + 1))
    rpc "{\"jsonrpc\":\"2.0\",\"method\":\"select_submodule\",\"params\":{\"repo_index\":$1,\"submodule_index\":$2},\"id\":$RPC_ID}"
}

rpc_set_tab() {
    RPC_ID=$((RPC_ID + 1))
    rpc "{\"jsonrpc\":\"2.0\",\"method\":\"set_tab\",\"params\":{\"tab\":\"$1\"},\"id\":$RPC_ID}"
}

wait_for_node() {
    local node_id="$1"
    local timeout="${2:-10}"
    for _ in $(seq 1 $((timeout * 2))); do
        local tree
        tree=$(get_tree)
        if echo "$tree" | python3 -c "
import json, sys
tree = json.loads(sys.stdin.read())['result']
def find(n, nid):
    if n.get('id') == nid: return n
    for c in n.get('children', []):
        r = find(c, nid)
        if r: return r
    return None
sys.exit(0 if find(tree, '$node_id') else 1)
" 2>/dev/null; then
            echo "$tree"
            return 0
        fi
        sleep 0.5
    done
    fail "Timed out waiting for node '$node_id'"
}

make_repo() {
    local name="$1" && shift
    local dir="$TEST_DIR/$name"
    mkdir -p "$dir"
    git -C "$dir" init -b main -q
    git -C "$dir" -c user.name="Test" -c user.email="test@test.local" commit --allow-empty -m "init $name" -q
    "$@" "$dir"
}

build_app() {
    echo "Building..."
    cargo build --features test-rpc --manifest-path "$PROJECT_DIR/Cargo.toml" -q 2>&1
}

launch_app() {
    local open_dir="$1"
    echo "Launching app..."
    GIT_MASTER_RPC_PORT="$RPC_PORT" GIT_MASTER_OPEN_DIR="$open_dir" "$PROJECT_DIR/target/debug/git_master" &
    APP_PID=$!
    sleep 1

    wait_for_rpc
    echo "RPC ready."
}
