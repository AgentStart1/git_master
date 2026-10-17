"""Task feedback remains bounded with real pending and failed Git commands."""
from pathlib import Path
import shutil
import json
import subprocess
import sys
import pytest
from driver import commit_all, find, git, status, wait_for
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
    app.scroll("status-details", -150)
    screenshot(app, "task-error-scrolled.png")
    # Keep Tk's delayed clipboard rendering out of the desktop driver process.
    subprocess.run([sys.executable, "-c",
        "import tkinter; r=tkinter.Tk(); r.withdraw(); r.clipboard_clear(); "
        "r.clipboard_append('unchanged clipboard sentinel'); r.update(); r.destroy()"],
        check=True, timeout=5)
    app.click("status-copy")
    def copied_output():
        # Read the OS clipboard through an independent UI connection.
        result = subprocess.run([sys.executable, "-c",
            "import tkinter,json; r=tkinter.Tk(); r.withdraw(); "
            "print(json.dumps(r.clipboard_get())); r.destroy()"],
            capture_output=True, text=True, timeout=5)
        return result.returncode == 0 and json.loads(result.stdout) == status(tree)
    wait_for(copied_output, description="complete diagnostic clipboard output")
    app.click("status-toggle")
    app.wait(lambda t: not find(t, "status-details"), "collapsed diagnostics")
    assert git(remote, "rev-parse", "main") == outgoing
    assert git(remote, "rev-parse", "main") != git(repo, "rev-parse", "HEAD")
