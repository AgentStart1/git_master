"""The logs window exposes a native draggable titlebar area."""
import sys
import pytest
from driver import find, make_repo, wait_for
from test_logs import open_logs, ready, closed


@pytest.mark.p1
@pytest.mark.skipif(sys.platform != "win32", reason="Native Windows titlebar dragging")
@pytest.mark.parametrize("title_y", [8, 24])
def test_logs_titlebar_blank_area_moves_window_and_preserves_controls(app_factory, isolated_git, title_y):
    import pyautogui
    root = isolated_git / "workspace"
    make_repo(root / "project")
    app = app_factory(root)
    logs = open_logs(app)
    ready(logs, "Application started")
    before = logs.window_geometry()
    # Real desktop input in the title's blank area, away from theme/window controls.
    x, y, width, height = before
    logical = find(logs.tree(), "logs-root")["bounds"]
    pyautogui.moveTo(x + width * 0.55, y + title_y * height / logical["height"])
    pyautogui.dragRel(80, 24, duration=0.5)
    def moved():
        current = logs.window_geometry()
        return current if current[0] >= x + 50 and current[1] >= y + 10 else None
    after = wait_for(moved, timeout=4, description="logs window moved by titlebar drag")
    assert after[2:] == before[2:], "dragging title must not resize the window"
    logs.click("logs-refresh")
    ready(logs, "Application started")
    logs.click("logs-close")
    closed(app)
