"""Mutation checks: swallowing an input must not pass on an old success/error."""
import sys
from types import SimpleNamespace
import pytest
from driver import App, status


def snapshot(message, revision=7, busy=False, prompt=False):
    return {"state": {"status_revision": revision, "busy": busy}, "children": [
        {"id": "status-message", "text": message},
        *([{"id": "test-prompt"}] if prompt else []),
    ]}


def observed_app(tree):
    app = App.__new__(App)
    app.tree = lambda: tree
    app.point = lambda node_id: (1, 1)
    def wait(predicate, description=""):
        value = app.tree()
        assert predicate(value), description
        return value
    app.wait = wait
    return app


@pytest.mark.parametrize("message", ["Git operation complete", "Git operation failed: index locked"])
def test_swallowed_operation_click_cannot_reuse_previous_status(monkeypatch, message):
    app = observed_app(snapshot(message))
    monkeypatch.setitem(sys.modules, "pyautogui", SimpleNamespace(click=lambda *a, **k: None))
    with pytest.raises(AssertionError, match="new operation completion"):
        app.click("stash-save")


@pytest.mark.parametrize("accept", [True, False])
def test_confirmation_disappearing_without_operation_does_not_pass(accept):
    app = observed_app(snapshot("Git operation failed: earlier failure", prompt=True))
    app.click = lambda node_id: setattr(app, "tree", lambda: snapshot("Git operation failed: earlier failure"))
    with pytest.raises(AssertionError, match="new operation completion"):
        app.prompt(accept)


def test_new_status_while_busy_is_not_completion():
    app = observed_app(snapshot("Running", revision=8, busy=True))
    with pytest.raises(AssertionError):
        app.wait_completion(7)


def test_new_idle_status_is_completion_and_path_cannot_supply_error():
    tree = snapshot("Git operation complete", revision=8)
    tree["children"].append({"text": "/test_failed/failed/repository"})
    app = observed_app(tree)
    assert app.wait_completion(7) is tree
    assert "failed" not in status(tree)


def test_linux_window_selection_matches_utf8_title_after_pid_search(monkeypatch):
    app = App.__new__(App)
    app.proc = SimpleNamespace(pid=123)
    app.window_title = "Git Master · 操作日志"
    calls = []

    def xdo(command, **kwargs):
        calls.append(command)
        args = command[1:]
        if args[0] == "search":
            assert args == ["search", "--onlyvisible", "--pid", "123"]
            return "10\n20\n"
        if args[0] == "getwindowname":
            return "Git Master\n" if args[1] == "10" else app.window_title + "\n"
        if args[0] == "getwindowgeometry":
            return "X=0\nY=0\nWIDTH=1440\nHEIGHT=1024\n"
        return ""

    monkeypatch.setattr(sys, "platform", "linux")
    monkeypatch.setattr("subprocess.check_output", xdo)
    assert app.window_geometry() == (0, 0, 1440, 1024)
    assert app.window == "20"
    assert ["xdotool", "windowfocus", "20"] in calls
