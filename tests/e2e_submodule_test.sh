#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/e2e_common.sh"
trap cleanup_e2e EXIT

# ── Setup: create repo with an uninitialized submodule ──

TEST_DIR=$(mktemp -d)
echo "Test repos dir: $TEST_DIR"

SOURCE_DIR="$TEST_DIR/_sources"
MODULE_SRC="$SOURCE_DIR/lib"
mkdir -p "$MODULE_SRC"
git -C "$MODULE_SRC" init -b main -q
git -C "$MODULE_SRC" -c user.name="Test" -c user.email="test@test.local" commit --allow-empty -m "init lib" -q

setup_delta() {
    local d="$1"
    git -C "$d" -c protocol.file.allow=always submodule add -q "$MODULE_SRC" modules/lib
    git -C "$d" -c user.name="Test" -c user.email="test@test.local" commit -m "add submodule" -q
    git -C "$d" submodule deinit -q -f modules/lib
    rm -rf "$d/modules/lib"
}

make_repo delta setup_delta

echo "Created repos: delta (uninitialized submodule)"

# ── Build & launch ──

build_app
launch_app "$TEST_DIR"

# ── Test 1: expand repo with uninitialized submodule ──

echo ""
echo "=== Test 1: Expand delta → Select submodule ==="
rpc_toggle_repo 0 > /dev/null
sleep 1

TREE=$(wait_for_node "repo-0-submodule-0" 10)

echo "$TREE" | python3 -c "
import json, sys
tree = json.loads(sys.stdin.read())['result']
def find(n, nid):
    if n.get('id') == nid: return n
    for c in n.get('children', []):
        r = find(c, nid)
        if r: return r
    return None

sub = find(tree, 'repo-0-submodule-0')
assert sub, 'submodule item not found'
texts = [c.get('text','') for c in sub.get('children',[]) if c.get('text')]
print(f'  Submodule item texts: {texts}')
assert 'modules/lib' in texts or 'lib' in texts, f'Expected submodule name, got: {texts}'
assert 'Not initialized' in texts, f'Expected uninitialized status, got: {texts}'
" || fail "Test 1 list"

rpc_select_submodule 0 0 > /dev/null
sleep 1

TREE2=$(wait_for_node "submodule-info-content" 10)

echo "$TREE2" | python3 -c "
import json, sys
tree = json.loads(sys.stdin.read())['result']
def find(n, nid):
    if n.get('id') == nid: return n
    for c in n.get('children', []):
        r = find(c, nid)
        if r: return r
    return None

info = find(tree, 'submodule-info-content')
assert info, 'submodule-info-content not found'
labels = [c.get('text','') for c in info.get('children',[]) if c.get('text')]
print(f'  Submodule info labels: {labels}')
assert any(l.startswith('Path:') and 'modules/lib' in l for l in labels), labels
assert 'Status: Not initialized' in labels, labels
assert find(tree, 'init-submodule-btn'), 'Initialize button not found'
" || fail "Test 1 detail"
pass "Uninitialized submodule can be expanded and selected"

# ── Done ──

echo ""
echo "==============================="
echo "  ALL TESTS PASSED"
echo "==============================="
