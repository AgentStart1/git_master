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
    logs.wait(lambda t: find(t, "logs-root"), "initial log window paint")
    logs.window_geometry()  # Xvfb must explicitly activate each new native window.
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
    tree = logs.wait(end_visible, "newest log initially followed")
    tree = scroll_up_and_pause(logs, 100)
    before = anchor(tree)[0]
    for _ in range(25):
        if end_visible(tree):
            break
        previous_index = anchor(tree)[0]
        # Keep each native wheel event within Windows' signed 16-bit delta.
        logs.scroll("logs-content", -100)
        tree = logs.wait(lambda t: end_visible(t) or anchor(t)[0] > previous_index,
            "log viewport moved after wheel input")
    assert end_visible(tree), "final log entry must be painted inside viewport"
    assert anchor(tree)[0] > before
    assert text(find(tree, "logs-end")) == marker
    screenshot(logs, "application-logs-scrolled.png")


def anchor(tree):
    node = find(tree, "logs-visible-anchor")
    assert node and node.get("bounds"), "a real fully visible log row must be painted"
    return node["state"]["index"], node["text"], node["bounds"]["y"]


def assert_same_anchor(tree, previous):
    current = anchor(tree)
    assert current[:2] == previous[:2], "the same displayed row must remain visible"
    assert abs(current[2] - previous[2]) < 1, "the paused painted row must not move"


def scroll_up_and_pause(logs, steps):
    logs.scroll("logs-content", steps)
    candidate = None
    since = None
    def settled(tree):
        nonlocal candidate, since
        if log_state(tree)["following"] or end_visible(tree):
            return False
        current = anchor(tree)
        ticks = log_state(tree)["poll_ticks"]
        if candidate != current:
            candidate, since = current, ticks
        # X11 emits one native event per notch. The first paused paint may
        # precede the rest of the batch. Establish the baseline before changing
        # files/releasing reads, across two real timer-driven painted frames.
        return ticks >= since + 2
    return logs.wait(settled, "native wheel batch settled before log update")


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
        tree = logs.wait(lambda t: log_state(t)["refresh_error"] is not None, "read error shown")
        assert "Cannot read" in text(find(tree, "logs-refresh-error"))
        assert find(tree, "logs-content")["text"] == "No logs recorded today."
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


def log_state(tree):
    return find(tree, "logs-panel")["state"]


def append_lines(path, lines):
    with path.open("a", encoding="utf-8") as log:
        log.write(lines)


@pytest.mark.p1
def test_logs_auto_refresh_follow_pause_and_resume(app_factory, isolated_git):
    app, _ = setup(app_factory, isolated_git)
    logs = open_logs(app)
    tree = ready(logs, "Application started")
    path = log_path(app, tree)
    marker = "AUTO_FOLLOW_" + uuid.uuid4().hex
    append_lines(path, "".join(f"reading entry {i}\n" for i in range(200)) + marker + "\n")
    tree = ready(logs, marker)  # No Refresh click.
    tree = logs.wait(end_visible, "automatic append followed into view")
    assert log_state(tree)["following"]
    tree = scroll_up_and_pause(logs, 20)
    original = find(tree, "logs-content")["text"]
    original_anchor = anchor(tree)
    completed = log_state(tree)["completed_reads"]
    next_marker = "PAUSED_APPEND_" + uuid.uuid4().hex
    append_lines(path, next_marker + "\n")
    tree = logs.wait(lambda t: log_state(t)["pending"] and log_state(t)["completed_reads"] > completed, "new logs read while follow paused")
    assert find(tree, "logs-content")["text"] == original
    assert_same_anchor(tree, original_anchor)
    screenshot(logs, "logs-follow-paused.png")
    logs.click("logs-follow")
    tree = ready(logs, next_marker)
    tree = logs.wait(lambda t: log_state(t)["following"] and end_visible(t), "follow latest button resumes and scrolls")
    assert not log_state(tree)["pending"]
    logs.scroll("logs-content", 20)
    logs.wait(lambda t: not log_state(t)["following"], "pause before natural scroll resume")
    append_lines(path, "SCROLL_RESUME_" + next_marker + "\n")
    logs.wait(lambda t: log_state(t)["pending"], "second append buffered")
    logs.scroll("logs-content", -100)
    tree = ready(logs, "SCROLL_RESUME_" + next_marker)
    tree = logs.wait(lambda t: log_state(t)["following"] and end_visible(t), "scroll to bottom resumes latest")
    screenshot(logs, "logs-follow-latest.png")


@pytest.mark.p1
def test_paused_logs_keep_position_when_bounded_tail_rolls(app_factory, isolated_git):
    app, _ = setup(app_factory, isolated_git)
    logs = open_logs(app)
    tree = ready(logs, "Application started")
    path = log_path(app, tree)
    prefix = uuid.uuid4().hex
    append_lines(path, "".join(f"ORIGINAL_{prefix}_{i}\n" for i in range(2100)))
    tree = ready(logs, f"ORIGINAL_{prefix}_2099")
    logs.wait(end_visible, "bounded initial tail followed")
    tree = scroll_up_and_pause(logs, 30)
    original = find(tree, "logs-content")["text"]
    original_anchor = anchor(tree)
    completed = log_state(tree)["completed_reads"]
    append_lines(path, "".join(f"ROLLED_{prefix}_{i}\n" for i in range(2200)))
    tree = logs.wait(lambda t: log_state(t)["pending"] and log_state(t)["completed_reads"] > completed, "rolled tail buffered")
    assert find(tree, "logs-content")["text"] == original
    assert_same_anchor(tree, original_anchor)
    logs.click("logs-follow")
    tree = ready(logs, f"ROLLED_{prefix}_2199")
    tree = logs.wait(end_visible, "latest rolled tail followed")
    assert "ORIGINAL_" + prefix not in find(tree, "logs-content")["text"]
    assert len(find(tree, "logs-content")["text"].splitlines()) <= 2000


@pytest.mark.p1
def test_auto_refresh_failure_retains_content_and_recovers(app_factory, isolated_git, monkeypatch):
    gate = isolated_git / "auto-error-gate"
    monkeypatch.setenv("GIT_MASTER_TEST_LOGS_GATE", str(gate))
    app, _ = setup(app_factory, isolated_git)
    logs = open_logs(app)
    tree = ready(logs, "Application started")
    path = log_path(app, tree)
    original = find(tree, "logs-content")["text"]
    gate.mkdir()
    try:
        wait_for(lambda: (gate / "entered").exists(), description="automatic log read held")
        path.unlink()
        path.mkdir()
    finally:
        (gate / "release").touch()
    try:
        tree = logs.wait(lambda t: log_state(t)["refresh_error"] is not None, "automatic read error surfaced")
        assert find(tree, "logs-content")["text"] == original
        assert "Cannot read" in text(find(tree, "logs-refresh-error"))
        assert 0 < find(tree, "logs-refresh-error")["bounds"]["height"] <= 24
    finally:
        path.rmdir()
    marker = "AUTO_RECOVERED_" + uuid.uuid4().hex
    path.write_text(marker + "\n", encoding="utf-8")
    tree = ready(logs, marker)
    assert log_state(tree)["refresh_error"] is None
    assert log_state(tree)["following"]


@pytest.mark.p1
@pytest.mark.parametrize("manual", [False, True], ids=["automatic", "manual"])
def test_paused_read_errors_preserve_content_and_viewport(app_factory, isolated_git, monkeypatch, manual):
    gate = isolated_git / "paused-error-gate"
    monkeypatch.setenv("GIT_MASTER_TEST_LOGS_GATE", str(gate))
    app, _ = setup(app_factory, isolated_git)
    logs = open_logs(app)
    tree = ready(logs, "Application started")
    path = log_path(app, tree)
    marker = "PAUSED_ERROR_" + uuid.uuid4().hex
    append_lines(path, "".join(f"paused error entry {i}\n" for i in range(200)) + marker + "\n")
    ready(logs, marker)
    logs.wait(end_visible, "ready to pause before read error")
    tree = scroll_up_and_pause(logs, 20)
    original = find(tree, "logs-content")["text"]
    original_anchor = anchor(tree)
    backup = path.with_suffix(".saved")
    gate.mkdir()
    try:
        wait_for(lambda: (gate / "entered").exists(), description="paused automatic read held")
        path.rename(backup)
        path.mkdir()
        if manual:
            logs.click("logs-refresh")
        completed = log_state(logs.tree())["completed_reads"]
        (gate / "release").touch()
        tree = logs.wait(lambda t: log_state(t)["completed_reads"] >= completed + (2 if manual else 1), "failed reads completed")
        screenshot(logs, f"paused-{manual}-read-error.png")
        assert find(tree, "logs-content")["text"] == original, "read errors must retain the paused snapshot"
        assert_same_anchor(tree, original_anchor)
        assert not log_state(tree)["following"]
        assert find(tree, "logs-refresh-error")["bounds"]["height"] <= 24
    finally:
        (gate / "release").touch()
        if path.is_dir():
            path.rmdir()
        if backup.exists():
            backup.rename(path)
    resumed = "RECOVER_PAUSED_" + uuid.uuid4().hex
    append_lines(path, resumed + "\n")
    tree = logs.wait(lambda t: log_state(t)["refresh_error"] is None and log_state(t)["pending"], "failed paused read recovers into buffer")
    assert find(tree, "logs-content")["text"] == original
    assert_same_anchor(tree, original_anchor)
    logs.click("logs-follow")
    ready(logs, resumed)
    logs.wait(end_visible, "recovered pending logs followed")


@pytest.fixture
def result_gate(isolated_git, monkeypatch):
    folder = isolated_git / "log-results"
    monkeypatch.setenv("GIT_MASTER_TEST_LOGS_RESULT_GATE", str(folder))
    class Gate:
        def arm(self, *tickets):
            folder.mkdir()
            for ticket in tickets:
                (folder / f"{ticket}.hold").touch()
            (folder / "armed").touch()
        def entered(self, ticket):
            wait_for(lambda: (folder / f"{ticket}.entered").exists(), description=f"snapshot {ticket} captured")
        def release(self, ticket):
            (folder / f"{ticket}.release").touch()
        def finished(self, ticket):
            wait_for(lambda: (folder / f"{ticket}.completed").exists(), description=f"snapshot {ticket} worker finished")
        def started(self, ticket):
            return (folder / f"{ticket}.entered").exists()
    gate = Gate()
    yield gate
    if folder.exists():
        for hold in folder.glob("*.hold"):
            (folder / (hold.stem + ".release")).touch()


@pytest.mark.p1
def test_closed_window_snapshot_arrives_after_reopened_window_is_ready(app_factory, isolated_git, result_gate):
    app, _ = setup(app_factory, isolated_git)
    old_logs = open_logs(app)
    tree = ready(old_logs, "Application started")
    old_generation = tree["state"]["window_generation"]
    path = log_path(app, tree)
    old_marker = "CLOSED_WINDOW_" + uuid.uuid4().hex
    path.write_text(old_marker + "\n", encoding="utf-8")
    ready(old_logs, old_marker)
    result_gate.arm(1)
    result_gate.entered(1)  # Snapshot contains old_marker before the window closes.
    old_logs.click("logs-close")
    closed(app)
    new_marker = "REOPENED_WINDOW_" + uuid.uuid4().hex
    path.write_text(new_marker + "\n", encoding="utf-8")
    logs = open_logs(app)
    tree = ready(logs, new_marker)  # New window is loaded while old result is held.
    generation = tree["state"]["window_generation"]
    assert generation > old_generation
    logs.window_geometry()  # Native window count must also be exactly one.
    logs.click("logs-follow")
    tree = logs.wait(lambda t: not log_state(t)["following"], "new window explicitly paused")
    completed = log_state(tree)["completed_reads"]
    result_gate.release(1)
    result_gate.finished(1)
    def unchanged_after_old_completion(t):
        assert t["state"]["window_generation"] == generation
        assert not log_state(t)["following"]
        content = find(t, "logs-content")["text"]
        assert new_marker in content and old_marker not in content
        return log_state(t)["completed_reads"] >= completed + 2
    logs.wait(unchanged_after_old_completion, "new window remains unchanged through later polls")
    screenshot(logs, "reopened-window-after-old-result.png")


@pytest.mark.p1
def test_queued_manual_refresh_is_serial_and_inflight_result_obeys_pause(app_factory, isolated_git, result_gate):
    app, _ = setup(app_factory, isolated_git)
    logs = open_logs(app)
    tree = ready(logs, "Application started")
    path = log_path(app, tree)
    first = "CAPTURED_" + uuid.uuid4().hex
    second = "QUEUED_" + uuid.uuid4().hex
    path.write_text("".join(f"inflight entry {i}\n" for i in range(200)) + first + "\n", encoding="utf-8")
    result_gate.arm(1, 2)
    result_gate.entered(1)
    tree = logs.wait(lambda t: log_state(t)["active_read"] is not None, "first inflight read painted")
    baseline = log_state(tree)["completed_reads"]
    ticks = log_state(tree)["poll_ticks"]
    active = log_state(tree)["active_read"]
    assert log_state(tree)["active_read_manual"] is False
    for _ in range(3):
        logs.click("logs-refresh")
    tree = logs.wait(lambda t: log_state(t)["poll_ticks"] >= ticks + 2, "timer ticks while first read is held")
    assert log_state(tree)["active_read"] == active
    assert log_state(tree)["completed_reads"] == baseline
    assert log_state(tree)["manual_queued"]
    assert not result_gate.started(2), "manual or automatic reads must not overlap"
    path.write_text("".join(f"queued entry {i}\n" for i in range(200)) + second + "\n", encoding="utf-8")
    result_gate.release(1)
    result_gate.entered(2)
    tree = ready(logs, first)
    tree = logs.wait(lambda t: log_state(t)["completed_reads"] == baseline + 1 and
        log_state(t)["active_read"] != active and not log_state(t)["manual_queued"], "one queued refresh starts after first result")
    assert log_state(tree)["active_read_manual"] is True, "a later automatic poll cannot stand in for queued manual refresh"
    assert not result_gate.started(3)
    logs.wait(end_visible, "first result followed before pausing inflight refresh")
    tree = scroll_up_and_pause(logs, 20)
    original_anchor = anchor(tree)
    result_gate.release(2)
    tree = logs.wait(lambda t: log_state(t)["completed_reads"] >= baseline + 2 and log_state(t)["pending"], "inflight queued result buffered using current follow state")
    assert find(tree, "logs-content")["text"].endswith(first)
    assert second not in find(tree, "logs-content")["text"]
    assert_same_anchor(tree, original_anchor)
    logs.click("logs-follow")
    ready(logs, second)
    logs.wait(end_visible, "queued result followed after explicit resume")
