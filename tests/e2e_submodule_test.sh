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

setup_epsilon() {
    local d="$1"
    git -C "$d" -c protocol.file.allow=always submodule add -q "$MODULE_SRC" modules/lib
    git -C "$d" -c user.name="Test" -c user.email="test@test.local" commit -m "add submodule" -q
}

make_repo delta setup_delta
make_repo epsilon setup_epsilon

echo "Created repos: delta (uninitialized submodule), epsilon (initialized submodule)"

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
pass "Uninitialized submodule appears in the repository list"

# ── Test 2: main/submodule commit canvas ──

echo ""
echo "=== Test 2: Main/submodule commit canvas ==="
rpc_select_repo 0 > /dev/null
sleep 1
rpc_set_tab "log" > /dev/null
rpc_set_log_view "canvas" > /dev/null
sleep 1

TREE2=$(wait_for_node "commit-canvas" 10)

echo "$TREE2" | python3 -c "
import json, sys
tree = json.loads(sys.stdin.read())['result']
def find(n, nid):
    if n.get('id') == nid: return n
    for c in n.get('children', []):
        r = find(c, nid)
        if r: return r
    return None

canvas = find(tree, 'commit-canvas')
assert canvas, 'commit-canvas not found'
lanes = [c for c in canvas.get('children', []) if c.get('node_type') == 'group']
assert len(lanes) == 2, f'Expected main + submodule lanes, got: {lanes}'
assert any(c.get('text') == 'delta' for c in lanes), lanes
assert any(c.get('text') in ('lib', 'modules/lib') for c in lanes), lanes

nodes = [c for c in canvas.get('children', []) if c.get('node_type') == 'commit-node']
placeholder_texts = [
    child.get('text', '')
    for node in nodes
    for child in node.get('children', [])
]
assert 'Referenced commit not loaded' in placeholder_texts, placeholder_texts

edges = [c.get('text', '') for c in canvas.get('children', []) if c.get('node_type') == 'graph-edge']
assert any('(submodule)' in edge for edge in edges), f'Submodule link missing: {edges}'
print('  Lanes:', [lane.get('text') for lane in lanes])
print('  Cross-repository edges:', [edge for edge in edges if '(submodule)' in edge])
" || fail "Test 2"
pass "Canvas links a main commit to its uninitialized submodule commit placeholder"

# ── Test 3: initialized submodule commit lane ──

echo ""
echo "=== Test 3: Initialized submodule commit lane ==="
rpc_select_repo 1 > /dev/null
sleep 1
rpc_set_tab "log" > /dev/null
rpc_set_log_view "canvas" > /dev/null
sleep 1

TREE3=$(wait_for_node "commit-canvas" 10)

echo "$TREE3" | python3 -c "
import json, sys
tree = json.loads(sys.stdin.read())['result']
def find(n, nid):
    if n.get('id') == nid: return n
    for c in n.get('children', []):
        r = find(c, nid)
        if r: return r
    return None

canvas = find(tree, 'commit-canvas')
assert canvas, 'commit-canvas not found'
lanes = [c for c in canvas.get('children', []) if c.get('node_type') == 'group']
assert len(lanes) == 2, f'Expected main + submodule lanes, got: {lanes}'
assert any(c.get('text') == 'epsilon' for c in lanes), lanes

nodes = [c for c in canvas.get('children', []) if c.get('node_type') == 'commit-node']
submodule_nodes = [node for node in nodes if node.get('id', '').startswith('commit-node-submodule:modules/lib-')]
assert submodule_nodes, f'Initialized submodule commit nodes missing: {nodes}'
texts = [child.get('text', '') for node in submodule_nodes for child in node.get('children', [])]
assert any('init lib' in text for text in texts), f'Submodule history missing: {texts}'
assert 'Referenced commit not loaded' not in texts, f'Loaded commit rendered as placeholder: {texts}'
print(f'  Initialized submodule commit nodes: {len(submodule_nodes)}')
" || fail "Test 3"
pass "Canvas renders initialized submodule commit history beside the main repository"

# ── Test 4: select initialized submodule → own canvas ──

echo ""
echo "=== Test 4: Selected submodule owns its canvas ==="
rpc_toggle_repo 1 > /dev/null
sleep 1
wait_for_node "repo-1-submodule-0" 10 > /dev/null
rpc_select_submodule 1 0 > /dev/null
sleep 1
rpc_set_tab "log" > /dev/null
rpc_set_log_view "canvas" > /dev/null
sleep 1

TREE4=$(wait_for_node "commit-canvas" 10)

echo "$TREE4" | python3 -c "
import json, sys
tree = json.loads(sys.stdin.read())['result']
def find(n, nid):
    if n.get('id') == nid: return n
    for c in n.get('children', []):
        r = find(c, nid)
        if r: return r
    return None

canvas = find(tree, 'commit-canvas')
assert canvas, 'commit-canvas not found'
lanes = [c.get('text') for c in canvas.get('children', []) if c.get('node_type') == 'group']
assert lanes == ['lib'], f'Expected only selected lib lane, got: {lanes}'
nodes = [c for c in canvas.get('children', []) if c.get('node_type') == 'commit-node']
texts = [child.get('text', '') for node in nodes for child in node.get('children', [])]
assert any('init lib' in text for text in texts), f'Submodule history missing: {texts}'
assert not any('add submodule' in text for text in texts), f'Parent history leaked into canvas: {texts}'
print('  Selected submodule lanes:', lanes)
" || fail "Test 4"
pass "Selected submodule canvas matches its own Git Log history"

# ── Test 5: select the uninitialized submodule ──

echo ""
echo "=== Test 5: Select uninitialized submodule ==="

rpc_select_submodule 0 0 > /dev/null
sleep 1

TREE5=$(wait_for_node "submodule-info-content" 10)

echo "$TREE5" | python3 -c "
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
" || fail "Test 5 detail"
pass "Uninitialized submodule can be selected"

# ── Done ──

echo ""
echo "==============================="
echo "  ALL TESTS PASSED"
echo "==============================="
