"""Open real application logs through desktop controls and verify bounded feedback."""
from pathlib import Path
import copy
import uuid
import sys
import pytest
from driver import find, text, make_repo, git, wait_for, assert_copied_text
from test_task_status import screenshot


def open_logs(app):
    app.window_title = "Git Master"
    app.click("open-logs-btn")
    logs = copy.copy(app)
    logs.window = None
    logs.window_title = "Git Master Logs"
    def tree():
        value = app.rpc("get_logs_view_tree")
        logs.last_tree = value
        return value
    logs.tree = tree
    return logs


def closed(app):
    return app.wait(lambda _: app.rpc("get_logs_view_tree") is None, "log window closed")


def ready(app, contains):
    return app.wait(lambda t: find(t, "logs-panel") and find(t, "logs-panel").get("state", {}).get("phase") == "ready"
        and contains in text(find(t, "logs-content") or {}), "displayed application logs")


def log_path(app, tree):
    path = Path(find(tree, "logs-panel")["state"]["path"]).resolve()
    assert path.is_relative_to((app.artifacts / "state").resolve()), "only isolated test logs may be changed"
    return path


def setup(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    app = app_factory(root)
    app.select(0, repo)
    return app, repo


@pytest.mark.p1
def test_application_logs_entry_refresh_copy_and_return(app_factory, isolated_git):
    app, repo = setup(app_factory, isolated_git)
    (repo / "tracked.txt").write_text("logged stage action\n", encoding="utf-8")
    app.tab("change")
    app.click("worktree-refresh")
    app.change(repo, "tracked.txt", "stage")
    logs = open_logs(app)
    tree = ready(logs, "worktree operation=stage END success=true")
    assert "Application started" in text(find(tree, "logs-content"))
    assert find(app.tree(), "worktree-content"), "main window stays on Git Change"
    app.click("open-logs-btn")
    logs.window_geometry()  # Exactly one independently titled log window must exist.
    path = log_path(app, tree)
    marker = "LOG_REFRESH_AND_COPY_" + uuid.uuid4().hex
    with path.open("a", encoding="utf-8") as log:
        log.write(marker + "\n")
    logs.click("logs-refresh")
    tree = ready(logs, marker)
    assert_copied_text(logs, "logs-copy", find(tree, "logs-content")["text"])
    screenshot(logs, "application-logs.png")
    logs.click("logs-close")
    closed(app)
    tree = app.wait(lambda t: find(t, "worktree-content"), "main Git Change remains available")
    assert tree["state"]["tab"] == "change"
    assert Path(tree["state"]["path"]).resolve() == repo.resolve()
    assert git(repo, "show", ":tracked.txt") == "logged stage action\n"


@pytest.mark.p1
def test_large_logs_are_bounded_and_really_scroll(app_factory, isolated_git):
    app, _ = setup(app_factory, isolated_git)
    logs = open_logs(app)
    tree = ready(logs, "Application started")
    path = log_path(app, tree)
    marker = "FINAL_LOG_ENTRY_" + uuid.uuid4().hex
    path.write_text("".join(f"ENTRY {i}: synthetic log content for scrolling\n" for i in range(12000)) + marker + "\n", encoding="utf-8")
    logs.click("logs-refresh")
    tree = ready(logs, marker)
    content = find(tree, "logs-content")["text"]
    assert len(content.encode("utf-8")) <= 256 * 1024
    assert len(content.splitlines()) <= 2000
    assert "ENTRY 0:" not in content
    assert find(tree, "logs-panel")["state"]["truncated"]
    assert not end_visible(tree)
    before = find(tree, "logs-end")["bounds"]["y"]
    for _ in range(25):
        if end_visible(tree):
            break
        previous_y = find(tree, "logs-end")["bounds"]["y"]
        # Keep each native wheel event within Windows' signed 16-bit delta.
        logs.scroll("logs-content", -100)
        tree = logs.wait(lambda t: find(t, "logs-end")["bounds"]["y"] < previous_y,
            "log viewport moved after wheel input")
    assert end_visible(tree), "final log entry must be painted inside viewport"
    assert find(tree, "logs-end")["bounds"]["y"] < before
    assert text(find(tree, "logs-end")) == marker
    screenshot(logs, "application-logs-scrolled.png")


def end_visible(tree):
    panel, last = find(tree, "logs-content"), find(tree, "logs-end")
    if not panel or not last or not last.get("bounds"):
        return False
    a, b = panel["bounds"], last["bounds"]
    return b["height"] > 0 and b["y"] >= a["y"] and b["y"] + b["height"] <= a["y"] + a["height"]


@pytest.mark.p1
def test_missing_logs_and_read_error_can_refresh_and_recover(app_factory, isolated_git):
    app, _ = setup(app_factory, isolated_git)
    logs = open_logs(app)
    tree = ready(logs, "Application started")
    path = log_path(app, tree)
    path.unlink()
    logs.click("logs-refresh")
    tree = ready(logs, "No logs recorded today.")
    assert not find(tree, "logs-copy")["interactive"]
    path.mkdir()
    try:
        logs.click("logs-refresh")
        tree = logs.wait(lambda t: find(t, "logs-panel").get("state", {}).get("phase") == "failed", "read error shown")
        assert "Cannot read" in text(find(tree, "logs-content"))
        assert find(tree, "logs-close")["bounds"]
        assert not find(tree, "logs-copy")["interactive"]
        screenshot(logs, "application-logs-read-error.png")
    finally:
        path.rmdir()
    path.write_text("RECOVERED_LOG_MARKER\n", encoding="utf-8")
    logs.click("logs-refresh")
    ready(logs, "RECOVERED_LOG_MARKER")


@pytest.mark.p1
def test_closing_loading_logs_does_not_reopen_on_completion(app_factory, isolated_git, monkeypatch):
    gate = isolated_git / "log-gate"
    monkeypatch.setenv("GIT_MASTER_TEST_LOGS_GATE", str(gate))
    app, repo = setup(app_factory, isolated_git)
    gate.mkdir()
    try:
        logs = open_logs(app)
        wait_for(lambda: (gate / "entered").exists(), description="log reader held")
        logs.wait(lambda t: find(t, "logs-panel").get("state", {}).get("phase") == "loading", "log read pending")
        logs.click("logs-close")
        closed(app)
    finally:
        (gate / "release").touch()
    logs = open_logs(app)
    ready(logs, "Application started")
    if sys.platform == "win32":
        import pyautogui
        logs.window_geometry()
        pyautogui.hotkey("alt", "f4")
    else:
        logs.click("logs-close")
    closed(app)
    logs = open_logs(app)
    ready(logs, "Application started")
    logs.click("logs-close")
    closed(app)
    assert Path(app.tree()["state"]["path"]).resolve() == repo.resolve()
