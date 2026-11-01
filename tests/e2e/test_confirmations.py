"""Exercise the production confirmation dialog with real keyboard and mouse input."""
from pathlib import Path
import shutil
import pytest
import pyautogui
from driver import find, git, make_repo, status, text
from test_failure_recovery import branching_repo
from test_task_status import screenshot


def focused(app, answer):
    return app.wait(lambda t: (prompt := find(t, "test-prompt")) and
        prompt.get("state", {}).get("focused_answer") == answer, "focused dialog button")


@pytest.fixture
def branch_confirmation(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    git(repo, "branch", "feature")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-delete-feature")
    focused(app, 1)
    return app, repo


@pytest.mark.p0
@pytest.mark.parametrize("key", ["enter", "space"])
def test_cancellation_consumes_held_activation_release_after_restoring_focus(branch_confirmation, key):
    app, repo = branch_confirmation
    revision = app.tree()["state"]["status_revision"]
    pyautogui.keyDown(key)
    try:
        pyautogui.press("esc")
        tree = app.wait_completion(revision)
        assert not find(tree, "test-prompt")
    finally:
        pyautogui.keyUp(key)
    app.tab("info")
    assert git(repo, "rev-parse", "--verify", "feature").strip()


@pytest.mark.p0
def test_alt_f4_cancels_dialog_and_preserves_the_main_window(branch_confirmation):
    app, repo = branch_confirmation
    revision = app.tree()["state"]["status_revision"]
    pyautogui.hotkey("alt", "f4")
    tree = app.wait_completion(revision)
    assert not find(tree, "test-prompt")
    assert "cancelled" in status(tree).lower()
    app.tab("info")
    assert git(repo, "rev-parse", "--verify", "feature").strip()


@pytest.mark.p1
@pytest.mark.parametrize("button", ["left", "middle", "right"])
def test_backdrop_mouse_buttons_preserve_keyboard_focus(branch_confirmation, button):
    app, repo = branch_confirmation
    revision = app.tree()["state"]["status_revision"]
    app.click("repo-0", button=button)
    focused(app, 1)
    pyautogui.press("enter")
    tree = app.wait_completion(revision)
    assert not find(tree, "test-prompt")
    assert "cancelled" in status(tree).lower()
    assert git(repo, "rev-parse", "--verify", "feature").strip()


@pytest.mark.p0
@pytest.mark.parametrize("key", ["enter", "space"])
def test_focus_change_during_a_held_key_does_not_activate_the_new_button(branch_confirmation, key):
    app, repo = branch_confirmation
    pyautogui.keyDown(key)
    try:
        pyautogui.press("tab")
        tree = focused(app, 0)
    finally:
        pyautogui.keyUp(key)
    revision = tree["state"]["status_revision"]
    pyautogui.press("esc")
    tree = app.wait_completion(revision)
    assert status(tree) == "Branch deletion cancelled"
    assert git(repo, "rev-parse", "--verify", "feature").strip()


@pytest.mark.p0
@pytest.mark.parametrize("key", ["esc", "enter", "space"])
def test_confirmation_defaults_to_cancel_and_restores_trigger_focus(app_factory, isolated_git, key):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    git(repo, "branch", "feature")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-delete-feature")
    tree = focused(app, 1)
    revision = tree["state"]["status_revision"]
    pyautogui.press(key)
    tree = app.wait_completion(revision)
    assert not find(tree, "test-prompt")
    assert "cancelled" in status(tree).lower()
    assert git(repo, "rev-parse", "--verify", "feature").strip()
    # Enter reopens the same focused trigger, proving focus was restored.
    pyautogui.press("enter")
    tree = focused(app, 1)
    revision = tree["state"]["status_revision"]
    pyautogui.press("esc")
    app.wait_completion(revision)
    assert git(repo, "rev-parse", "--verify", "feature").strip()


@pytest.mark.p0
@pytest.mark.parametrize("key", ["enter", "space"])
def test_confirmation_keyboard_selection_deletes_only_after_explicit_focus(app_factory, isolated_git, key):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    git(repo, "branch", "feature")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-delete-feature")
    focused(app, 1)
    pyautogui.press("tab")
    tree = focused(app, 0)
    revision = tree["state"]["status_revision"]
    assert git(repo, "rev-parse", "--verify", "feature").strip()
    pyautogui.press(key)
    tree = app.wait_completion(revision)
    assert not find(tree, "test-prompt")
    assert status(tree) == "Branch operation complete"
    assert git(repo, "show-ref", "--verify", "refs/heads/feature", check=False).returncode != 0


@pytest.mark.p1
def test_confirmation_traps_focus_and_blocks_background_clicks(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "alpha")
    other = make_repo(root / "beta")
    git(repo, "branch", "feature")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-delete-feature")
    focused(app, 1)
    for expected in (0, 1, 0, 1):
        pyautogui.press("tab")
        focused(app, expected)
    pyautogui.hotkey("shift", "tab")
    focused(app, 0)
    pyautogui.hotkey("shift", "tab")
    focused(app, 1)
    app.click("repo-1")
    tree = focused(app, 1)
    assert Path(tree["state"]["path"]).resolve() == repo.resolve()
    assert git(other, "branch", "--show-current").strip() == "main"
    pyautogui.press("esc")
    app.wait(lambda t: app.idle(t) and not find(t, "test-prompt"))


@pytest.mark.p0
@pytest.mark.parametrize("confirm_force", [False, True])
def test_force_confirmation_starts_on_cancel_after_keyboard_safe_deletion(app_factory, isolated_git, confirm_force):
    root, repo = branching_repo(isolated_git)
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-delete-feature")
    focused(app, 1)
    pyautogui.press("tab")
    focused(app, 0)
    pyautogui.press("enter")
    app.wait(lambda t: "Force delete local branch 'feature'?" in text(find(t, "test-prompt") or {}))
    tree = focused(app, 1)
    assert git(repo, "rev-parse", "--verify", "feature").strip()
    if confirm_force:
        pyautogui.press("tab")
        tree = focused(app, 0)
    revision = tree["state"]["status_revision"]
    pyautogui.press("enter")
    tree = app.wait_completion(revision)
    exists = git(repo, "show-ref", "--verify", "refs/heads/feature", check=False).returncode == 0
    assert exists is not confirm_force
    assert status(tree) == ("Branch operation complete" if confirm_force else "Branch deletion cancelled")


@pytest.mark.p1
def test_long_confirmation_keeps_buttons_visible_and_defaults_to_cancel(app_factory, isolated_git):
    root, repo = branching_repo(isolated_git)
    git(repo, "branch", "finished")
    hook = repo / ".git" / "hooks" / "reference-transaction"
    shutil.copyfile(Path(__file__).parent / "fixtures" / "verbose_failure_hook.sh", hook)
    hook.chmod(0o755)
    # A merged branch reaches the rejecting hook and produces lengthy diagnostics.
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-delete-finished")
    app.click("test-prompt-answer-0")
    tree = app.wait(lambda t: "Force delete local branch 'finished'?" in
        text(find(t, "test-prompt") or {}), "force confirmation with diagnostics")
    tree = focused(app, 1)
    assert "FINAL_DIAGNOSTIC_MARKER" in text(find(tree, "test-prompt"))
    for index in (0, 1):
        app.point(f"test-prompt-answer-{index}")
    screenshot(app, "long-confirmation.png")
    pyautogui.press("end")
    app.wait(lambda t: find(t, "test-prompt")["state"]["detail_scroll_y"] < -100, "keyboard scrolling to end")
    focused(app, 1)
    pyautogui.press("home")
    tree = app.wait(lambda t: find(t, "test-prompt")["state"]["detail_scroll_y"] == 0, "keyboard scrolling to start")
    revision = tree["state"]["status_revision"]
    pyautogui.press("esc")
    app.wait_completion(revision)
    assert git(repo, "rev-parse", "--verify", "finished").strip()
