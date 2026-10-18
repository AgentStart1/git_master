"""Native resize and wheel input against the list's actual clipped viewport."""
import ctypes
import subprocess
import sys
import uuid
import pytest
from driver import find
from test_logs import (setup, open_logs, ready, log_path, log_state, result_gate,
                       scroll_up_and_pause, settled_paused_scroll, anchor, assert_same_anchor, end_visible)
from test_task_status import screenshot


def resize(logs, width, height):
    x, y, physical_width, physical_height = logs.window_geometry()
    root = find(logs.tree(), "logs-root")["bounds"]
    scale = physical_width / root["width"]
    if sys.platform == "win32":
        from ctypes import wintypes
        user32 = ctypes.windll.user32
        rectangle = wintypes.RECT()
        user32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
        assert user32.GetWindowRect(logs.window, ctypes.byref(rectangle))
        frame_width = rectangle.right - rectangle.left - physical_width
        frame_height = rectangle.bottom - rectangle.top - physical_height
        assert user32.SetWindowPos(logs.window, 0, 0, 0,
            round(width * scale) + frame_width, round(height * scale) + frame_height, 0x0002 | 0x0004)
    else:
        subprocess.run(["xdotool", "windowsize", str(logs.window),
            str(round(width * scale)), str(round(height * scale))], check=True, timeout=3)
    tree = logs.wait(lambda t: abs(find(t, "logs-root")["bounds"]["width"] - width) < 1 and
        abs(find(t, "logs-root")["bounds"]["height"] - height) < 1, "native client size painted")
    viewport = find(tree, "logs-viewport")["bounds"]
    assert 0 < viewport["width"] <= width and 0 < viewport["height"] <= height
    assert 0 <= viewport["x"] and viewport["x"] + viewport["width"] <= width + 0.5
    assert 0 <= viewport["y"] and viewport["y"] + viewport["height"] <= height + 0.5
    return tree


def tail_visible(tree):
    viewport, tail = find(tree, "logs-viewport"), find(tree, "logs-end")
    if not viewport or not tail or not tail.get("bounds"):
        return False
    a, b = viewport["bounds"], tail["bounds"]
    return b["height"] > 0 and a["y"] < b["y"] + b["height"] <= a["y"] + a["height"] + 0.5


def top(tree):
    row = find(tree, "logs-top-row")
    assert row and row.get("bounds"), "a real partially visible row must be painted"
    viewport = find(tree, "logs-viewport")["bounds"]
    bounds = row["bounds"]
    fraction = max(0, viewport["y"] - bounds["y"]) / bounds["height"]
    return row["state"]["index"], row["text"], fraction, bounds["height"]


@pytest.mark.p1
def test_small_downward_scroll_after_pausing_initial_read_does_not_follow(
        app_factory, isolated_git, result_gate):
    app, _ = setup(app_factory, isolated_git)
    paths = [path for path in (app.artifacts / "state" / "GitMaster" / "logs").glob("operations-*.log")
        if f"pid={app.proc.pid} Application started" in path.read_text(encoding="utf-8")]
    assert len(paths) == 1
    marker = uuid.uuid4().hex
    data = "".join(f"START_{marker}_{i}\n" for i in range(1600))
    paths[0].write_text(data, encoding="utf-8")
    result_gate.arm(1, 2)
    logs = open_logs(app)
    result_gate.entered(1)
    logs.click("logs-follow")
    logs.wait(lambda t: not log_state(t)["following"], "pause initial read")
    result_gate.release(1)
    tree = ready(logs, f"START_{marker}_1599")
    assert not log_state(tree)["following"] and top(tree)[0] < 5
    before = tree["state"]["view_revision"]
    logs.scroll("logs-content", -8)
    tree = logs.wait(lambda t: t["state"]["view_revision"] > before and top(t)[0] > 0,
        "small downward input painted near start")
    assert top(tree)[0] < 100
    assert not tail_visible(tree)
    assert not log_state(tree)["following"], "measured-area bottom is not the end of the log"
    ticks = log_state(tree)["poll_ticks"]
    tree = logs.wait(lambda t: log_state(t)["poll_ticks"] >= ticks + 2, "remain paused through later frames")
    assert not log_state(tree)["following"] and top(tree)[0] < 100


@pytest.mark.p1
def test_paused_wrapped_row_keeps_reading_fraction_across_width_changes(
        app_factory, isolated_git, result_gate):
    app, _ = setup(app_factory, isolated_git)
    logs = open_logs(app)
    path = log_path(app, ready(logs, "Application started"))
    marker = uuid.uuid4().hex
    path.write_text("".join(f"ROW_{marker}_{i:04}: " + "word " * 90 + "\n" for i in range(40)), encoding="utf-8")
    ready(logs, f"ROW_{marker}_0039")
    resize(logs, 600, 650)
    logs.wait(tail_visible, "narrow long rows followed")
    tree = scroll_up_and_pause(logs, 30)
    for _ in range(8):
        if top(tree)[0] < 30:
            break
        tree = scroll_up_and_pause(logs, 100)
    assert top(tree)[0] < 30, "resize must happen far from tail clamping"
    for _ in range(10):
        if top(tree)[2] >= 0.65:
            break
        logs.scroll("logs-content", -1)
        tree = settled_paused_scroll(logs)
    original = top(tree)
    assert original[2] >= 0.65, "test must start inside a wrapped row"
    result_gate.arm(1)
    result_gate.entered(1)  # A later read must not repair resize state.
    completed = log_state(logs.tree())["completed_reads"]
    for width, height in [(1100, 650), (600, 650), (850, 450)]:
        tree = resize(logs, width, height)
        assert log_state(tree)["completed_reads"] == completed
        current = top(tree)
        assert not log_state(tree)["following"]
        assert current[:2] == original[:2], "resizing must not skip the row being read"
        assert abs(current[2] - original[2]) <= 19.2 / current[3] + 0.03
        if width == 1100:
            assert current[3] < original[3], "the original wrapped row must actually reflow"
    screenshot(logs, "paused-wide-wrapped-row.png")


@pytest.mark.p1
def test_following_resizes_keep_the_actual_tail_visible_without_new_reads(
        app_factory, isolated_git, result_gate):
    app, _ = setup(app_factory, isolated_git)
    logs = open_logs(app)
    path = log_path(app, ready(logs, "Application started"))
    marker = "WRAPPED_TAIL_" + uuid.uuid4().hex
    path.write_text("".join(f"line {i}: {'word ' * 80}\n" for i in range(350)) + "word " * 90 + marker + "\n", encoding="utf-8")
    ready(logs, marker)
    logs.wait(tail_visible, "long final entry initially followed")
    result_gate.arm(1)
    result_gate.entered(1)
    completed = log_state(logs.tree())["completed_reads"]
    for width, height in [(650, 250), (1100, 650), (750, 450), (1000, 700)]:
        tree = resize(logs, width, height)
        assert log_state(tree)["completed_reads"] == completed
        assert log_state(tree)["following"]
        assert tail_visible(tree), "the resized paint must include the real final wrapped row"
        if height == 250:
            assert find(tree, "logs-end")["bounds"]["height"] > find(tree, "logs-viewport")["bounds"]["height"], "test must cover an entry taller than the viewport"
            screenshot(logs, "following-tall-final-row.png")
    screenshot(logs, "following-resize.png")


@pytest.mark.p1
def test_rapid_wheel_reversals_keep_last_upward_intent_and_buffered_position(
        app_factory, isolated_git, result_gate):
    import pyautogui
    app, _ = setup(app_factory, isolated_git)
    logs = open_logs(app)
    path = log_path(app, ready(logs, "Application started"))
    marker = uuid.uuid4().hex
    original = "".join(f"BURST_{marker}_{i}: {'word ' * 35}\n" for i in range(300))
    path.write_text(original, encoding="utf-8")
    ready(logs, f"BURST_{marker}_299")
    logs.wait(tail_visible, "initial burst tail followed")
    result_gate.arm(1, 2, 3)
    result_gate.entered(1)
    baseline = log_state(logs.tree())["completed_reads"]
    latest = "AFTER_BURST_" + marker
    path.write_text(original + latest + "\n", encoding="utf-8")
    pyautogui.moveTo(*logs.point("logs-content", (0.8, 0.7)))
    old_pause = pyautogui.PAUSE
    try:
        pyautogui.PAUSE = 0
        for steps in (-3, 20, -3, 20):
            pyautogui.scroll(steps * 120 if sys.platform == "win32" else steps)
    finally:
        pyautogui.PAUSE = old_pause
    tree = settled_paused_scroll(logs)
    position = anchor(tree)
    content = find(tree, "logs-content")["text"]
    result_gate.release(1)
    result_gate.entered(2)
    result_gate.release(2)
    def unchanged(t):
        assert not log_state(t)["following"]
        assert find(t, "logs-content")["text"] == content
        assert_same_anchor(t, position)
        return log_state(t)["completed_reads"] == baseline + 2 and log_state(t)["pending"]
    logs.wait(unchanged, "rapid-scroll intent survives buffered read publication")
    for _ in range(30):
        logs.scroll("logs-content", -100)
        tree = logs.tree()
        if log_state(tree)["following"]:
            break
    ready(logs, latest)
    logs.wait(tail_visible, "real bottom resumes latest buffered result")
    screenshot(logs, "rapid-wheel-resumed.png")


@pytest.mark.p1
def test_tail_in_padding_is_not_reported_as_visible(app_factory, isolated_git, result_gate):
    app, _ = setup(app_factory, isolated_git)
    logs = open_logs(app)
    path = log_path(app, ready(logs, "Application started"))
    marker = "CLIPPED_TAIL_" + uuid.uuid4().hex
    path.write_text("".join(f"padding row {i}\n" for i in range(200)) + marker + "\n", encoding="utf-8")
    ready(logs, marker)
    logs.wait(tail_visible, "tail initially inside actual viewport")
    logs.click("logs-follow")
    logs.wait(lambda t: not log_state(t)["following"], "explicit pause at bottom")
    result_gate.arm(1)
    result_gate.entered(1)
    tree = resize(logs, 1000, 692)
    tail = find(tree, "logs-end")["bounds"]
    outer = find(tree, "logs-content")["bounds"]
    viewport = find(tree, "logs-viewport")["bounds"]
    bottom = tail["y"] + tail["height"]
    assert viewport["y"] + viewport["height"] < bottom <= outer["y"] + outer["height"], "fixture must place the tail inside padding but outside the clip"
    assert not end_visible(tree), "padding is not painted log content"
    assert not log_state(tree)["following"]
    screenshot(logs, "tail-clipped-by-padding.png")
