"""Real daily files and replacements, with later polling unable to hide bad results."""
import uuid
import pytest
from driver import find
from test_logs import (setup, open_logs, ready, log_path, log_state, end_visible,
                       result_gate)
from test_task_status import screenshot


def set_date(path, value):
    # The poller must see either complete date, never a partially rewritten file.
    replacement = path.with_suffix(".next")
    replacement.write_text(value, encoding="utf-8")
    replacement.replace(path)


@pytest.mark.p1
@pytest.mark.parametrize("mode", ["rename", "copy-truncate", "new-year"])
@pytest.mark.parametrize("paused", [False, True], ids=["following", "paused"])
def test_rotation_replaces_the_tail_and_preserves_follow_mode(
        app_factory, isolated_git, monkeypatch, result_gate, mode, paused):
    date = isolated_git / "log-date"
    set_date(date, "2030-12-31")
    monkeypatch.setenv("GIT_MASTER_TEST_LOG_DATE", str(date))
    app, _ = setup(app_factory, isolated_git)
    logs = open_logs(app)
    tree = ready(logs, "Application started")
    path = log_path(app, tree)
    assert path.name == "operations-2030-12-31.log"
    prefix = uuid.uuid4().hex
    # Both limits apply to the initial snapshot; replacement is deliberately short.
    path.write_text("".join(f"OLD_{prefix}_{i}: {'中' * 50}\n" for i in range(3000)), encoding="utf-8")
    tree = ready(logs, f"OLD_{prefix}_2999")
    logs.wait(end_visible, "old truncated tail is followed")
    assert log_state(tree)["truncated"]
    if paused:
        logs.scroll("logs-content", 30)
        tree = logs.wait(lambda t: not log_state(t)["following"] and not end_visible(t), "paused before rotation")
    original = find(tree, "logs-content")["text"]
    original_y = find(tree, "logs-end")["bounds"]["y"]
    result_gate.arm(1, 2, 3)
    result_gate.entered(1)  # A captured pre-rotation result must publish first.
    baseline = log_state(logs.tree())["completed_reads"]
    replacement = f"NEW_{prefix}\nFINAL_{prefix}"
    destination = path
    if mode == "rename":
        path.rename(path.with_suffix(f".{prefix}.rotated"))
    elif mode == "new-year":
        destination = path.with_name("operations-2031-01-01.log")
    destination.write_text(replacement + "\n", encoding="utf-8")
    if mode == "new-year":
        set_date(date, "2031-01-01")
    result_gate.release(1)
    result_gate.entered(2)  # The real post-rotation read has finished on its worker.
    result_gate.release(2)
    tree = logs.wait(lambda t: log_state(t)["completed_reads"] == baseline + 2, "first post-rotation result painted")
    # Ticket 3 is held: a later successful poll cannot repair a broken first result.
    if paused:
        assert not log_state(tree)["following"]
        assert log_state(tree)["pending"]
        assert log_path(app, tree) == path
        assert find(tree, "logs-content")["text"] == original
        assert abs(find(tree, "logs-end")["bounds"]["y"] - original_y) < 1
        logs.click("logs-follow")
        tree = logs.wait(lambda t: log_state(t)["following"], "resume buffered replacement")
    assert find(tree, "logs-content")["text"] == replacement
    assert log_path(app, tree) == destination
    assert not log_state(tree)["truncated"]
    assert not log_state(tree)["pending"]
    logs.wait(end_visible, "short replacement followed after dropping truncation notice")
    screenshot(logs, "rotated-tail.png")


@pytest.mark.p1
def test_missing_new_day_file_then_automatic_writer_uses_new_date(
        app_factory, isolated_git, monkeypatch):
    date = isolated_git / "log-date"
    set_date(date, "2030-12-31")
    monkeypatch.setenv("GIT_MASTER_TEST_LOG_DATE", str(date))
    app, repo = setup(app_factory, isolated_git)
    logs = open_logs(app)
    tree = ready(logs, "Application started")
    yesterday = log_path(app, tree)
    old = yesterday.read_text(encoding="utf-8")
    today = yesterday.with_name("operations-2031-01-01.log")
    # Test artifacts can survive earlier runs; require a genuinely absent daily file.
    today.unlink(missing_ok=True)
    set_date(date, "2031-01-01")
    tree = logs.wait(lambda t: log_state(t).get("path") == str(today) and
        find(t, "logs-content")["text"] == "No logs recorded today.", "missing new-day file automatically selected")
    assert not today.exists()
    assert not find(tree, "logs-copy")["interactive"]
    assert log_state(tree)["following"]
    (repo / "tracked.txt").write_text("stage after midnight\n", encoding="utf-8")
    app.tab("change")
    app.click("worktree-refresh")
    app.change(repo, "tracked.txt", "stage")
    logs.window_geometry()  # Return to the native log window before observing its paint.
    tree = ready(logs, "worktree operation=stage END success=true")
    assert log_path(app, tree) == today
    assert "Application started" not in find(tree, "logs-content")["text"]
    assert yesterday.read_text(encoding="utf-8") == old
    assert "worktree operation=stage END success=true" in today.read_text(encoding="utf-8")
    logs.wait(end_visible, "new-day writer entry automatically followed")


@pytest.mark.p1
def test_oversized_unicode_final_entry_remains_readable_and_followed(
        app_factory, isolated_git, result_gate):
    app, _ = setup(app_factory, isolated_git)
    logs = open_logs(app)
    tree = ready(logs, "Application started")
    path = log_path(app, tree)
    result_gate.arm(1, 2, 3)
    result_gate.entered(1)
    baseline = log_state(logs.tree())["completed_reads"]
    marker = "LONG_END_" + uuid.uuid4().hex
    data = "old\n" + "中" * 100_000 + marker + "\n"
    path.write_bytes(data.encode("utf-8"))
    # Independently compute the exact suffix on UTF-8 scalar boundaries.
    expected = data.encode("utf-8")[-256 * 1024:].decode("utf-8", errors="ignore").rstrip("\n")
    result_gate.release(1)
    result_gate.entered(2)
    result_gate.release(2)
    tree = logs.wait(lambda t: log_state(t)["completed_reads"] == baseline + 2, "first oversized-entry result painted")
    assert find(tree, "logs-content")["text"] == expected
    assert log_state(tree)["truncated"]
    assert log_state(tree)["following"]
    assert find(tree, "logs-copy")["interactive"]
    def final_wrapped_row_visible(t):
        viewport = find(t, "logs-content")["bounds"]
        entry = find(t, "logs-end")["bounds"]
        bottom = entry["y"] + entry["height"]
        # The last logical entry wraps over many screens; its final visual row,
        # rather than the entire entry, must be inside the painted viewport.
        return viewport["y"] < bottom <= viewport["y"] + viewport["height"]
    logs.wait(final_wrapped_row_visible, "oversized final entry followed")
    screenshot(logs, "oversized-final-entry.png")
