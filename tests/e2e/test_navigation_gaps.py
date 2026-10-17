"""Core read, refresh and graph interactions missing from the first E2E suite."""
import pytest
from driver import commit_all, find, git, make_repo, status, text, walk
from test_sync import remote_fixture


@pytest.mark.p1
def test_empty_workspace_and_clean_repo_refresh_external_changes(app_factory, isolated_git):
    empty = isolated_git / "empty"
    empty.mkdir()
    app = app_factory(empty)
    assert not find(app.tree(), "repo-0")
    app.click("fetch-all-btn")
    app.click("switch-all-main-btn")
    assert app.idle(app.tree())
    app.close()
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("change")
    assert "Working tree clean" in text(app.tree())
    (repo / "external.txt").write_text("external edit\n", encoding="utf-8")
    app.click("worktree-refresh")
    app.wait(lambda t: app.idle(t) and "external.txt" in text(t), "external change refreshed")
    assert "●" in text(find(app.tree(), "repo-0"))
    app.tab("info")
    assert "1 new" in text(find(app.tree(), "info-content"))
    app.tab("stash")
    assert "No stashes" in text(app.tree())
    app.click("stash-save")
    app.wait(lambda t: app.idle(t) and "Git operation complete" in text(t), "untracked-only stash saved")
    oid = git(repo, "rev-parse", "refs/stash").strip()
    app.click(f"stash-select-{oid}")
    app.wait(lambda t: app.idle(t) and "external edit" in text(find(t, "worktree-diff") or {}), "stash title preview")
    assert "Staged changes" not in text(find(app.tree(), "worktree-diff"))
    assert "Working tree changes" not in text(find(app.tree(), "worktree-diff"))
    assert "Untracked files" in text(find(app.tree(), "worktree-diff"))


@pytest.mark.p1
@pytest.mark.parametrize("kind", ["renamed", "binary", "unicode"])
def test_special_file_preview_stage_unstage(app_factory, isolated_git, kind):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    if kind == "renamed":
        git(repo, "mv", "tracked.txt", "renamed.txt")
        relative = "renamed.txt"
    elif kind == "binary":
        relative = "binary.dat"
        (repo / relative).write_bytes(b"\x00\x01binary\xff\x00")
    else:
        relative = "中文 file.txt"
        (repo / relative).write_text("中文\told → new\n", encoding="utf-8")
    before = (repo / relative).read_bytes()
    app = app_factory(root)
    app.select(0, repo)
    app.tab("change")
    app.change(repo, relative, "diff")
    app.wait(lambda t: app.idle(t) and text(find(t, "worktree-diff") or {}), "special preview")
    preview = text(find(app.tree(), "worktree-diff"))
    if kind == "binary":
        assert "binary" in preview.lower()
    elif kind == "unicode":
        assert "中文" in preview and "→" in preview
    else:
        assert "Staged" in preview
    if kind != "renamed":
        app.change(repo, relative, "stage")
        app.wait(lambda t: app.idle(t) and any(n.get("id", "").startswith("change-unstage-") for n in walk(t)), "staged file")
    app.change(repo, relative, "unstage")
    app.wait(lambda t: app.idle(t) and "Git operation complete" in text(t), "unstage completed")
    assert (repo / relative).read_bytes() == before
    assert relative not in git(repo, "diff", "--cached", "--name-only", "-z").split("\0")
    if kind == "renamed":
        assert git(repo, "diff", "--cached", "--name-only") == "", "unstage rename must restore both index paths"


@pytest.mark.p1
def test_branch_filter_changes_list_and_graph_without_checkout(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    git(repo, "checkout", "-b", "feature")
    commit_all(repo, "Feature-only history")
    git(repo, "checkout", "main")
    commit_all(repo, "Main-only history")
    original = git(repo, "rev-parse", "HEAD").strip()
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    # Make selection deterministic: the current branch is visible by default.
    app.click("log-branch-feature")
    app.wait(lambda t: "Git Log updated" in text(t) and "Feature-only history" in text(find(t, "log-list") or {}), "added feature history")
    app.click("log-branch-main")
    app.wait(lambda t: "Main-only history" not in text(find(t, "log-list") or {}), "hidden main history")
    app.click("log-branch-feature")
    app.wait(lambda t: "At least one branch must remain visible" in text(t), "last branch safeguard")
    assert "Feature-only history" in text(find(app.tree(), "log-list"))
    app.click("log-view-canvas")
    canvas = find(app.wait(lambda t: find(t, "commit-canvas"), "filtered canvas"), "commit-canvas")
    assert "Feature-only history" in text(canvas)
    assert "Main-only history" not in text(canvas)
    assert git(repo, "rev-parse", "HEAD").strip() == original
    assert git(repo, "branch", "--show-current").strip() == "main"


@pytest.mark.p1
def test_canvas_click_drag_pan_zoom_and_per_repo_view_state(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "alpha")
    other = make_repo(root / "beta")
    oid = git(repo, "rev-parse", "HEAD").strip()
    node_id = f"commit-node-main-{oid}"
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-view-canvas")
    initial = app.wait(lambda t: find(t, node_id) and find(t, node_id).get("bounds"), "painted canvas node")
    original = find(initial, node_id)["bounds"]
    app.click(node_id)
    app.wait(lambda t: find(t, "commit-details") and oid in text(find(t, "commit-details")), "canvas commit details")
    app.click("close-commit-details")
    app.wait(lambda t: not find(t, "commit-details"), "closed details")
    app.drag(node_id, 35, 30)
    moved = app.wait(lambda t: abs(find(t, node_id)["bounds"]["y"] - original["y"]) > 10, "node drag")
    assert not find(moved, "commit-details"), "drag must not open details"
    before = find(moved, node_id)["bounds"]
    app.drag("commit-canvas", 20, -20, fraction=(0.9, 0.7))
    app.wait(lambda t: abs(find(t, node_id)["bounds"]["y"] - before["y"]) > 5, "background pan")
    app.scroll("commit-canvas", 5)
    zoomed = app.wait(lambda t: abs(find(t, node_id)["bounds"]["width"] - before["width"]) > 2, "wheel zoom")
    saved = find(zoomed, node_id)["bounds"]
    app.select(1, other)
    app.select(0, repo)
    app.tab("log")
    app.click("log-view-canvas")
    restored = find(app.wait(lambda t: find(t, node_id) and find(t, node_id).get("bounds"), "restored canvas"), node_id)["bounds"]
    assert restored == pytest.approx(saved, abs=1)
    assert git(repo, "status", "--porcelain") == ""


@pytest.mark.p1
def test_context_menu_right_click_preserves_selection_and_outside_click_recovers(app_factory, isolated_git):
    root = isolated_git / "workspace"
    first, second = make_repo(root / "alpha"), make_repo(root / "beta")
    app = app_factory(root)
    app.select(0, first)
    app.click("repo-1", button="right")
    app.wait(lambda t: find(t, "context-menu"), "repository context menu")
    for node_id in ("ctx-open-directory", "ctx-open-terminal"):
        assert find(app.tree(), node_id)["interactive"]
    assert app.tree()["state"]["path"] == str(first)
    app.click("repo-0")
    app.wait(lambda t: app.idle(t) and not find(t, "context-menu") and t["state"]["path"] == str(first),
        "dismissed context menu and completed clicked repository reload")
    assert app.tree()["state"]["path"] == str(first)
    app.select(1, second)


@pytest.mark.p1
def test_fetch_failure_reports_error_then_recovers(app_factory, isolated_git):
    root, repo, remote, _ = remote_fixture(isolated_git)
    original = git(repo, "rev-parse", "HEAD").strip()
    git(repo, "remote", "set-url", "origin", str(isolated_git / "missing.git"))
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-remote-origin-fetch")
    app.wait(lambda t: app.idle(t) and "failed" in status(t).lower(), "fetch failure")
    assert git(repo, "rev-parse", "HEAD").strip() == original
    git(repo, "remote", "set-url", "origin", str(remote))
    app.click("log-remote-origin-fetch")
    app.wait(lambda t: app.idle(t) and "Fetch complete" in text(t), "fetch retry")


@pytest.mark.p1
@pytest.mark.parametrize("mode", ["master", "remote-default"])
def test_batch_main_resolves_non_main_default_branches(app_factory, isolated_git, mode):
    root, repo, remote, seed = remote_fixture(isolated_git)
    target = "master" if mode == "master" else "trunk"
    git(seed, "branch", "-m", target)
    git(seed, "push", "origin", target)
    git(remote, "symbolic-ref", "HEAD", f"refs/heads/{target}")
    git(repo, "fetch", "origin")
    git(repo, "checkout", "-b", "feature")
    git(repo, "branch", "-D", "main")
    if mode == "master":
        git(repo, "branch", "master", "origin/master")
        git(repo, "symbolic-ref", "--delete", "refs/remotes/origin/HEAD")
    else:
        git(repo, "remote", "set-head", "origin", "--auto")
    app = app_factory(root)
    app.click("switch-all-main-btn")
    app.wait(lambda t: app.idle(t) and "1 succeeded, 0 failed" in text(t), "resolved default branch")
    assert git(repo, "branch", "--show-current").strip() == target
    assert git(repo, "status", "--porcelain") == ""
