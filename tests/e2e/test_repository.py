from pathlib import Path
import pytest
from driver import commit_all, find, git, make_repo, status, text, walk


@pytest.mark.p1
def test_scan_selection_history_list_canvas_and_commit_details(app_factory, isolated_git):
    root = isolated_git / "workspace"
    alpha = make_repo(root / "alpha")
    commit_all(alpha, "Add feature A")
    commit_all(alpha, "Fix bug B")
    beta = make_repo(root / "beta")
    (beta / "dirty.txt").write_text("local\n", encoding="utf-8")
    gamma = make_repo(root / "gamma")
    git(gamma, "checkout", "-b", "dev")
    app = app_factory(root)
    tree = app.tree()
    for index, name in enumerate(("alpha", "beta", "gamma")):
        assert name in text(find(tree, f"repo-{index}"))
    assert "●" in text(find(tree, "repo-1"))
    assert "Branch: dev" in text(find(tree, "repo-2"))
    app.select(0, alpha)
    assert "Branch: main" in text(find(app.tree(), "info-content"))
    app.tab("log")
    tree = app.wait(lambda tree: find(tree, "log-list"), "history list")
    for message in ("Base commit", "Add feature A", "Fix bug B"):
        assert message in text(find(tree, "log-list"))
    oid = git(alpha, "rev-parse", "HEAD").strip()
    app.click(f"log-commit-{oid}")
    tree = app.wait(lambda tree: (node := find(tree, "commit-details")) and oid in text(node) and "Fix bug B" in text(node), "commit details")
    app.click("close-commit-details")
    app.wait(lambda tree: not find(tree, "commit-details"), "closed commit details")
    app.click("log-view-canvas")
    tree = app.wait(lambda tree: find(tree, "commit-canvas"), "canvas")
    canvas = find(tree, "commit-canvas")
    assert "alpha" in text(canvas)
    assert len([node for node in walk(canvas) if node["node_type"] == "commit-node"]) >= 3
    assert "Fix bug B" in text(canvas)
    app.select(1, beta)
    app.tab("change")
    assert "dirty.txt" in text(app.tree())
    app.select(0, alpha)
    app.tab("change")
    assert "Working tree clean" in text(app.tree())
    assert "dirty.txt" not in text(find(app.tree(), "worktree-content"))


@pytest.mark.p1
def test_submodule_placeholders_own_history_and_initialization(app_factory, isolated_git):
    source = make_repo(isolated_git / "sources" / "lib")
    root = isolated_git / "workspace"
    delta = make_repo(root / "delta")
    epsilon = make_repo(root / "epsilon")
    for repo in (delta, epsilon):
        git(repo, "submodule", "add", str(source), "modules/lib")
        commit_all(repo, "Add submodule")
    git(delta, "submodule", "deinit", "-f", "modules/lib")
    app = app_factory(root)
    app.click("repo-0-toggle")
    tree = app.wait(lambda tree: find(tree, "repo-0-submodule-0"), "expanded uninitialized submodule")
    assert "Not initialized" in text(find(tree, "repo-0-submodule-0"))
    app.select(0, delta)
    app.tab("log")
    app.click("log-view-canvas")
    tree = app.wait(lambda tree: find(tree, "commit-canvas"), "parent canvas")
    canvas = find(tree, "commit-canvas")
    assert len([n for n in canvas["children"] if n["node_type"] == "group"]) == 2
    assert "Referenced commit not loaded" in text(canvas)
    assert "(submodule)" in text(canvas) and "(head)" in text(canvas)
    app.select(1, epsilon)
    app.tab("log")
    app.click("log-view-canvas")
    tree = app.wait(lambda tree: find(tree, "commit-canvas"), "initialized submodule canvas")
    nodes = [n for n in walk(find(tree, "commit-canvas")) if n.get("id", "").startswith("commit-node-submodule:")]
    assert nodes and "Base commit" in "\n".join(text(n) for n in nodes)
    app.click("repo-1-toggle")
    app.click("repo-1-submodule-0")
    module = epsilon / "modules" / "lib"
    app.wait(lambda tree: app.idle(tree) and tree["state"].get("path") and Path(tree["state"]["path"]).resolve() == module.resolve(), "selected submodule")
    app.tab("log")
    app.click("log-view-canvas")
    tree = app.wait(lambda tree: find(tree, "commit-canvas"), "submodule-owned canvas")
    lanes = [n.get("text") for n in find(tree, "commit-canvas")["children"] if n["node_type"] == "group"]
    assert lanes == ["lib"]
    assert "Add submodule" not in text(find(tree, "commit-canvas"))
    (module / "tracked.txt").write_text("submodule local edit\n", encoding="utf-8")
    app.tab("change")
    app.change(module, "tracked.txt", "stage")
    app.wait(lambda tree: app.idle(tree) and "Staged" in text(tree), "submodule staging")
    assert git(module, "diff", "--cached", "--name-only").strip() == "tracked.txt"
    assert git(epsilon, "diff", "--cached", "--name-only") == ""
    app.click("repo-0-submodule-0")
    tree = app.wait(lambda tree: find(tree, "submodule-info-content"), "uninitialized details")
    assert "Status: Not initialized" in text(tree)
    app.click("init-submodule-btn")
    app.wait(lambda tree: app.idle(tree) and find(tree, "info-content"), "submodule initialized")
    assert git(delta / "modules" / "lib", "rev-parse", "HEAD").strip() == git(source, "rev-parse", "HEAD").strip()


@pytest.mark.p0
def test_branch_checkout_and_unmerged_deletion_preserve_data(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    git(repo, "checkout", "-b", "feature")
    (repo / "tracked.txt").write_text("feature branch\n", encoding="utf-8")
    commit_all(repo, "Feature commit")
    git(repo, "checkout", "main")
    (repo / "tracked.txt").write_text("unsaved main work\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-checkout-feature")
    app.wait(lambda tree: app.idle(tree) and "failed" in status(tree).lower(), "checkout refuses overlapping changes")
    assert git(repo, "branch", "--show-current").strip() == "main"
    assert (repo / "tracked.txt").read_text(encoding="utf-8") == "unsaved main work\n"
    app.click("log-delete-feature")
    app.prompt(False)
    assert git(repo, "rev-parse", "--verify", "feature").strip()
    app.click("log-delete-feature")
    app.prompt(True)
    assert "not fully merged" in status(app.tree()).lower()
    assert git(repo, "rev-parse", "--verify", "feature").strip()
    assert (repo / "tracked.txt").read_text(encoding="utf-8") == "unsaved main work\n"


@pytest.mark.p1
def test_branch_switch_and_delete_merged_branch(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    git(repo, "branch", "feature")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-checkout-feature")
    app.wait(lambda tree: app.idle(tree) and find(tree, "log-checkout-main"), "checkout feature")
    assert git(repo, "branch", "--show-current").strip() == "feature"
    app.click("log-checkout-main")
    app.wait(lambda tree: app.idle(tree) and find(tree, "log-checkout-feature"), "checkout main")
    app.click("log-delete-feature")
    app.prompt(True)
    assert git(repo, "rev-parse", "--verify", "feature", check=False).returncode != 0
