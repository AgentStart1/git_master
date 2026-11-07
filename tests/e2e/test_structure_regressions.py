"""Target ownership across held input, asynchronous reads and navigation."""
from pathlib import Path
import pytest
from driver import commit_all, find, git, make_repo, text, wait_for
from test_sync import remote_fixture
from test_async_races import gate, select_while_busy


@pytest.mark.p1
@pytest.mark.parametrize("action", ["copy", "refresh", "reset"])
def test_remote_menu_action_survives_held_mouse_press(app_factory, isolated_git, action):
    import pyautogui
    root, repo, _, _ = remote_fixture(isolated_git)
    app = app_factory(root)
    app.select(0, repo)
    app.click("remote-more-origin")
    node = "log-remote-origin-reset" if action == "reset" else f"remote-{action}-origin"
    point = app.point(node)
    before = app.tree()["state"]["view_revision"]
    pyautogui.mouseDown(*point)
    try:
        app.wait(lambda t: t["state"]["view_revision"] > before, "frame while menu item held")
        assert (app.node(node) or {}).get("bounds"), "menu unmounted during press"
    finally:
        pyautogui.mouseUp()
    if action == "reset":
        app.wait(lambda t: find(t, "test-prompt"), "reset confirmation")
        app.prompt(False)
    elif action == "refresh":
        app.wait(lambda t: not t["state"]["loading_detail"] and not (find(t, node) or {}).get("bounds"), "local refresh completed")
        assert Path(app.tree()["state"]["path"]).resolve() == repo.resolve()
    else:
        app.wait(lambda t: not (find(t, node) or {}).get("bounds"), "copy closed menu")
        # Read the native clipboard after this real input action.
        import subprocess, sys, json
        value = subprocess.check_output([sys.executable, "-c", "import tkinter,json; r=tkinter.Tk();r.withdraw();print(json.dumps(r.clipboard_get()));r.destroy()"], text=True, timeout=5)
        assert json.loads(value).endswith(".git")


@pytest.mark.p1
@pytest.mark.parametrize("kind", ["file", "stash"])
def test_new_preview_target_hides_old_content_while_read_is_held(app_factory, isolated_git, monkeypatch, kind):
    folder = isolated_git / "read-gate"
    monkeypatch.setenv("GIT_MASTER_TEST_WORKTREE_READ_GATE", str(folder))
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    if kind == "file":
        for name, content in [("a.txt", "ONLY_OLD_PREVIEW"), ("b.txt", "ONLY_NEW_PREVIEW")]:
            (repo / name).write_text(content + "\n", encoding="utf-8")
        first, second = "change-diff-0", "change-diff-1"
    else:
        ids = []
        for content in ["ONLY_OLD_PREVIEW", "ONLY_NEW_PREVIEW"]:
            (repo / "tracked.txt").write_text(content + "\n", encoding="utf-8")
            git(repo, "stash", "push", "-m", content)
            ids.append(git(repo, "rev-parse", "refs/stash").strip())
        first, second = ["stash-select-" + oid for oid in ids]
    app = app_factory(root)
    app.select(0, repo)
    app.tab("change" if kind == "file" else "stash")
    app.click(first)
    assert "ONLY_OLD_PREVIEW" in text(find(app.tree(), "worktree-diff"))
    folder.mkdir()
    try:
        app.click(second, await_operation=False)
        wait_for(lambda: (folder / "entered").exists(), description="second preview held")
        tree = app.wait(lambda t: t["state"]["worktree_loading"], "loading target")
        assert "ONLY_OLD_PREVIEW" not in text(find(tree, "worktree-diff"))
        assert "Loading preview" in text(find(tree, "worktree-diff"))
        pyautogui = __import__("pyautogui")
        pyautogui.screenshot().save(app.artifacts / "pending-target.png")
    finally:
        (folder / "release").touch()
    app.wait(lambda t: not t["state"]["worktree_loading"] and "ONLY_NEW_PREVIEW" in text(find(t, "worktree-diff") or {}), "new target published")


@pytest.mark.p1
@pytest.mark.parametrize("read", ["detail", "worktree"])
def test_same_workspace_rescan_keeps_pending_read_owned_after_reindex(app_factory, isolated_git, monkeypatch, read):
    folder = isolated_git / "read-gate"
    key = "GIT_MASTER_TEST_DETAIL_READ_GATE" if read == "detail" else "GIT_MASTER_TEST_WORKTREE_READ_GATE"
    monkeypatch.setenv(key, str(folder))
    root = isolated_git / "workspace"
    repo = make_repo(root / "z-target")
    (repo / "pending.txt").write_text("pending target data\n", encoding="utf-8")
    app = app_factory(root)
    if read == "worktree":
        app.select(0, repo)
        app.tab("change")
    folder.mkdir()
    try:
        app.click("repo-0" if read == "detail" else "change-diff-0", await_operation=False)
        wait_for(lambda: (folder / "entered").exists(), description="read held before rescan")
        make_repo(root / "a-new")
        app.click("workspace-menu")
        app.click("recent-workspace-0", await_operation=False)
        app.wait(lambda t: not t["state"]["scanning"] and "a-new" in text(find(t, "repo-list") or t), "same workspace rescanned")
    finally:
        (folder / "release").touch()
    app.wait(lambda t: not t["state"]["loading_detail"] and not t["state"]["worktree_loading"] and t["state"].get("path") and Path(t["state"]["path"]).resolve() == repo.resolve(), "original request completes after repository reorder")
    if read == "worktree":
        assert "pending target data" in text(find(app.tree(), "worktree-diff"))


@pytest.mark.p1
def test_failed_push_after_navigation_recovers_original_repository(app_factory, isolated_git, gate):
    root, repo, _, _ = remote_fixture(isolated_git)
    other = make_repo(root / "second")
    git(other, "checkout", "-b", "other-branch")
    (repo / "outgoing.txt").write_text("original target\n", encoding="utf-8")
    commit_all(repo, "outgoing")
    (repo / "original-only.txt").write_text("keep original work\n", encoding="utf-8")
    gate.install(repo, "pre-push")
    hook = repo / ".git" / "hooks" / "pre-push"
    hook.write_text(hook.read_text() + "\nexit 1\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.click("log-remote-origin-push", await_operation=False)
    gate.entered()
    select_while_busy(app, 1, other)
    gate.release()
    tree = app.wait(lambda t: not t["state"]["busy"] and find(t, "blocked-context"), "original push failure shown")
    context = text(find(tree, "blocked-context"))
    assert str(repo) in context and "other-branch" not in context
    assert Path(tree["state"]["path"]).resolve() == other.resolve()
    app.click("blocked-recover", await_operation=False)
    app.wait(lambda t: app.idle(t) and t["state"]["tab"] == "change" and t["state"].get("path") and Path(t["state"]["path"]).resolve() == repo.resolve(), "recovery navigates to original repository")
    assert "original-only.txt" in text(find(app.tree(), "worktree-content"))


@pytest.mark.p1
@pytest.mark.parametrize("menu", ["workspace", "remote", "picker", "reference"])
def test_popup_escape_restores_trigger_keyboard_focus(app_factory, isolated_git, menu):
    import pyautogui
    from test_focus_visibility import top_border_pixels
    root, repo, _, _ = remote_fixture(isolated_git)
    app = app_factory(root)
    app.select(0, repo)
    app.click("theme-dark")
    if menu == "reference": app.tab("info")
    trigger, child = {
        "workspace": ("workspace-menu", "change-dir-btn"),
        "remote": ("remote-more-origin", "remote-copy-origin"),
        "picker": ("remote-picker", "remote-select-origin"),
        "reference": ("overview-reference-picker", "overview-reference-origin/main"),
    }[menu]
    app.click(trigger)
    app.wait(lambda t: (find(t, child) or {}).get("bounds"), "menu open before keyboard dismissal")
    pyautogui.press("tab")
    pyautogui.press("esc")
    app.wait(lambda t: not (find(t, child) or {}).get("bounds"), "Escape dismissed popup")
    wait_for(lambda: top_border_pixels(app, trigger, (114, 174, 230)) >= 12, description="focus returned to popup trigger")
