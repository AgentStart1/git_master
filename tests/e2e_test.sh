#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/e2e_common.sh"
trap cleanup_e2e EXIT

# ── Setup: create fake git repos ──

TEST_DIR=$(mktemp -d)
echo "Test repos dir: $TEST_DIR"

setup_alpha() {
    local d="$1"
    git -C "$d" -c user.name="Test" -c user.email="test@test.local" commit --allow-empty -m "Add feature A" -q
    git -C "$d" -c user.name="Test" -c user.email="test@test.local" commit --allow-empty -m "Fix bug B" -q
}

setup_beta() {
    local d="$1"
    echo "wip" > "$d/todo.txt"
}

setup_gamma() {
    local d="$1"
    git -C "$d" checkout -b dev -q
    git -C "$d" -c user.name="Test" -c user.email="test@test.local" commit --allow-empty -m "dev work" -q
}

make_repo alpha setup_alpha
make_repo beta  setup_beta
make_repo gamma setup_gamma

echo "Created repos: alpha (3 commits), beta (dirty), gamma (branch dev)"

# ── Build & launch ──

build_app
launch_app "$TEST_DIR"

# ── Test 1: repo list ──

echo ""
echo "=== Test 1: Repo list ==="
TREE=$(get_tree)

echo "$TREE" | python3 -c "
import json, sys
tree = json.loads(sys.stdin.read())['result']
def find(n, nid):
    if n.get('id') == nid: return n
    for c in n.get('children', []):
        r = find(c, nid)
        if r: return r
    return None

repo_list = find(tree, 'repo-list')
assert repo_list, 'repo-list node not found'

items = [c for c in repo_list.get('children', []) if c['node_type'] == 'list-item']
names = []
for item in items:
    texts = [c.get('text','') for c in item.get('children',[]) if c.get('text')]
    if texts:
        names.append(texts[0])

print(f'  Repos found: {names}')
assert 'alpha' in names, f'alpha not in {names}'
assert 'beta'  in names, f'beta not in {names}'
assert 'gamma' in names, f'gamma not in {names}'

# Check beta is dirty
beta_idx = names.index('beta')
beta_item = items[beta_idx]
indicators = [c.get('text','') for c in beta_item.get('children',[])]
assert '●' in indicators, f'beta should be dirty, indicators: {indicators}'
print('  beta is dirty: OK')

# Check gamma is on dev branch
gamma_idx = names.index('gamma')
gamma_item = items[gamma_idx]
gamma_texts = [c.get('text','') for c in gamma_item.get('children',[])]
assert 'dev' in gamma_texts, f'gamma should be on dev branch, texts: {gamma_texts}'
print('  gamma branch=dev: OK')
" || fail "Test 1"
pass "Repo list shows all repos with correct status"

# ── Test 2: select repo → Info tab ──

echo ""
echo "=== Test 2: Select alpha → Info tab ==="
rpc_select_repo 0 > /dev/null
sleep 1

TREE2=$(wait_for_node "info-content" 10)

echo "$TREE2" | python3 -c "
import json, sys
tree = json.loads(sys.stdin.read())['result']
def find(n, nid):
    if n.get('id') == nid: return n
    for c in n.get('children', []):
        r = find(c, nid)
        if r: return r
    return None

info = find(tree, 'info-content')
assert info, 'info-content not found'

labels = [c.get('text','') for c in info.get('children',[]) if c.get('text')]
print(f'  Info labels: {labels}')

path_labels = [l for l in labels if l.startswith('Path:')]
assert path_labels, f'No Path label found in {labels}'
assert 'alpha' in path_labels[0], f'Expected alpha in path, got: {path_labels[0]}'

branch_labels = [l for l in labels if l.startswith('Branch:')]
assert branch_labels, 'No Branch label'
assert 'main' in branch_labels[0], f'Expected main branch, got: {branch_labels[0]}'
print('  Path contains alpha: OK')
print('  Branch is main: OK')
" || fail "Test 2"
pass "Info tab shows alpha repo details (path + branch)"

# ── Test 3: switch to Git Log tab ──

echo ""
echo "=== Test 3: Switch to Git Log tab ==="
rpc_set_tab "log" > /dev/null
sleep 1

TREE3=$(wait_for_node "log-content" 10)

echo "$TREE3" | python3 -c "
import json, sys
tree = json.loads(sys.stdin.read())['result']
def find(n, nid):
    if n.get('id') == nid: return n
    for c in n.get('children', []):
        r = find(c, nid)
        if r: return r
    return None

log = find(tree, 'log-content')
assert log, 'log-content not found'

entries = log.get('children', [])
print(f'  Log entries: {len(entries)}')
assert len(entries) >= 3, f'Expected >= 3 log entries, got {len(entries)}'

messages = []
for entry in entries:
    texts = [c.get('text','') for c in entry.get('children',[]) if c.get('text')]
    if len(texts) >= 2:
        messages.append(texts[1])

print(f'  Messages: {messages}')
assert any('Fix bug B' in m for m in messages), f'\"Fix bug B\" not in log: {messages}'
assert any('Add feature A' in m for m in messages), f'\"Add feature A\" not in log: {messages}'
assert any('init alpha' in m for m in messages), f'\"init alpha\" not in log: {messages}'
" || fail "Test 3"
pass "Git Log tab shows 3 commits for alpha"

# ── Done ──

echo ""
echo "==============================="
echo "  ALL TESTS PASSED"
echo "==============================="
