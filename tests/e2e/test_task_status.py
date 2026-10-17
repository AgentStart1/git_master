"""Task feedback remains bounded with real pending and failed Git commands."""
from pathlib import Path
import shutil
import pytest
from driver import commit_all, find, git, status, wait_for, text, make_repo, walk, assert_copied_text
from test_async_races import gate
from test_sync import remote_fixture


def screenshot(app, name):
    import pyautogui
    x, y, width, height = app.window_geometry()
    pyautogui.screenshot(region=(x, y, width, height)).save(app.artifacts / name)


@pytest.mark.p1
def test_pending_task_and_verbose_failure_keep_content_visible(app_factory, isolated_git, gate):
    root, repo, remote, _ = remote_fixture(isolated_git)
    (repo / "outgoing.txt").write_text("local commit\n", encoding="utf-8")
    commit_all(repo, "Outgoing task feedback")
    outgoing = git(repo, "rev-parse", "HEAD")
    gate.install(repo, "pre-push")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-remote-origin-push", await_operation=False)
    gate.entered()
    tree = app.wait(lambda t: t["state"]["busy"] and find(t, "status-summary").get("bounds"), "painted task progress")
    assert find(tree, "top-bar")["bounds"]["height"] < 120
    assert find(tree, "main-content")["bounds"]["height"] > 300
    screenshot(app, "task-running.png")
    revision = tree["state"]["status_revision"]
    app.click("status-toggle")
    app.wait(lambda t: find(t, "status-details"), "pending task details")
    gate.release()
    app.wait_completion(revision)
    assert not find(app.tree(), "status-details"), "new result must collapse previous details"
    assert git(remote, "rev-parse", "main") == outgoing

    hook = repo / ".git" / "hooks" / "pre-push"
    shutil.copyfile(Path(__file__).parent / "fixtures" / "verbose_failure_hook.sh", hook)
    hook.chmod(0o755)
    (repo / "outgoing.txt").write_text("second local commit\n", encoding="utf-8")
    commit_all(repo, "Rejected push")
    app.click("log-remote-origin-push")
    tree = app.tree()
    assert "failed" in status(tree).lower()
    assert "FINAL_DIAGNOSTIC_MARKER" in status(tree)
    assert not find(tree, "status-details")
    assert find(tree, "top-bar")["bounds"]["height"] < 120
    assert find(tree, "main-content")["bounds"]["height"] > 300
    screenshot(app, "task-error-collapsed.png")
    app.click("status-toggle")
    tree = app.wait(lambda t: find(t, "status-details") and find(t, "status-details").get("bounds"), "expanded diagnostics")
    assert find(tree, "status-details")["bounds"]["height"] <= 161
    assert find(tree, "main-content")["bounds"]["height"] > 250
    screenshot(app, "task-error-expanded.png")
    assert not detail_end_visible(tree), "last diagnostic must start outside the viewport"
    before_scroll = find(tree, "status-detail-end")["bounds"]["y"]
    app.scroll("status-details", -150)
    tree = app.wait(lambda t: detail_end_visible(t), "last diagnostic painted inside viewport")
    assert find(tree, "status-detail-end")["bounds"]["y"] < before_scroll
    assert text(find(tree, "status-detail-end")) == status(tree).splitlines()[-1]
    screenshot(app, "task-error-scrolled.png")
    assert_copied_text(app, "status-copy", status(tree))
    app.click("status-toggle")
    app.wait(lambda t: not find(t, "status-details"), "collapsed diagnostics")
    assert git(remote, "rev-parse", "main") == outgoing
    assert git(remote, "rev-parse", "main") != git(repo, "rev-parse", "HEAD")


def detail_end_visible(tree):
    panel = find(tree, "status-details")
    end = find(tree, "status-detail-end")
    if not panel or not end or not panel.get("bounds") or not end.get("bounds"):
        return False
    viewport, line = panel["bounds"], end["bounds"]
    return line["height"] > 0 and line["y"] >= viewport["y"] and (
        line["y"] + line["height"] <= viewport["y"] + viewport["height"])


@pytest.mark.p1
@pytest.mark.parametrize("previous", ["success", "failure"])
@pytest.mark.parametrize("action, expected", [
    ("stage", "Staging tracked.txt…"), ("unstage", "Unstaging tracked.txt…"),
    ("save", "Saving stash…"), ("apply", "Applying stash…"),
    ("pop", "Popping stash…"), ("drop", "Deleting stash…"),
], ids=["stage", "unstage", "save", "apply", "pop", "drop"])
def test_worktree_task_replaces_old_result_and_details(app_factory, isolated_git, monkeypatch, action, expected, previous):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    path = repo / "tracked.txt"
    path.write_text("pending operation content\n", encoding="utf-8")
    stash_action = action in ("apply", "pop", "drop")
    if stash_action:
        git(repo, "stash", "push", "-m", "Progress candidate")
        oid = git(repo, "rev-parse", "refs/stash").strip()
    if action == "unstage":
        git(repo, "add", "tracked.txt")
    gate_folder = isolated_git / "worktree-gate"
    monkeypatch.setenv("GIT_MASTER_TEST_WORKTREE_GATE", str(gate_folder))
    app = app_factory(root)
    app.select(0, repo)
    app.tab("change" if action in ("stage", "unstage") else "stash")
    if previous == "failure":
        error_file = repo / "error-only.txt"
        error_file.write_text("temporary dirty content\n", encoding="utf-8")
        lock = repo / ".git" / "index.lock"
        file_action = action in ("stage", "unstage")
        if file_action:
            lock.touch()
        else:
            git(repo, "config", "--unset", "user.name")
            git(repo, "config", "--unset", "user.email")
            git(repo, "config", "user.useConfigOnly", "true")
        before_failure = git(repo, "status", "--porcelain")
        try:
            if file_action:
                app.change(repo, "tracked.txt", action)
            else:
                app.click("stash-save")
            assert "failed" in status(app.tree()).lower()
            assert git(repo, "status", "--porcelain") == before_failure
        finally:
            if file_action:
                lock.unlink()
            else:
                git(repo, "config", "user.name", "E2E Tests")
                git(repo, "config", "user.email", "e2e@example.invalid")
            error_file.unlink()
    else:
        assert "refreshed" in status(app.tree())
    old_status = status(app.tree())
    app.click("status-toggle")
    app.wait(lambda t: find(t, "status-details"), "old result expanded")
    gate_folder.mkdir()
    try:
        revision = app.tree()["state"]["status_revision"]
        if action in ("stage", "unstage"):
            entry = next(n["state"] for n in walk(app.tree()) if n.get("state", {}).get("path") == "tracked.txt")
            app.click(f"change-{action}-{entry['index']}", await_operation=False)
        elif action == "save":
            app.click("stash-save", await_operation=False)
        else:
            app.click(f"stash-{action}-{oid}", await_operation=False)
            if action == "drop":
                app.prompt(True, await_operation=False)
        wait_for(lambda: (gate_folder / "entered").exists(), description="worktree worker held")
        tree = app.wait(lambda t: t["state"]["busy"] and find(t, "status-summary").get("bounds"),
            "painted worktree progress")
        assert text(find(tree, "status-summary")) == expected
        assert status(tree) != old_status
        assert not find(tree, "status-details")
        assert tree["state"]["status_revision"] > revision
        screenshot(app, f"worktree-{action}-{previous}-pending.png")
        revision = tree["state"]["status_revision"]
    finally:
        (gate_folder / "release").touch()
    tree = app.wait_completion(revision)
    assert status(tree) == "Git operation complete"
    if action == "stage":
        assert git(repo, "show", ":tracked.txt") == "pending operation content\n"
    elif action == "unstage":
        assert git(repo, "diff", "--cached", "--name-only") == ""
        assert path.read_text(encoding="utf-8") == "pending operation content\n"
    elif action == "save":
        assert git(repo, "status", "--porcelain") == ""
        assert git(repo, "stash", "list")
    else:
        assert path.read_text(encoding="utf-8") == ("pending operation content\n" if action != "drop" else "base\n")
        assert bool(git(repo, "stash", "list")) == (action == "apply")
