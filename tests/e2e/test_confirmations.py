"""Confirm the current safe-deletion dialog with real keyboard input."""
import pytest
import pyautogui
from driver import find, git, make_repo, status


@pytest.fixture
def branch_confirmation(app_factory, isolated_git):
    repo = make_repo(isolated_git / "workspace" / "project")
    git(repo, "branch", "feature")
    app = app_factory(repo.parent)
    app.select(0, repo)
    app.tab("log")
    app.click("log-delete-feature")
    app.wait(lambda tree: find(tree, "test-prompt"), "branch confirmation")
    return app, repo


@pytest.mark.p0
@pytest.mark.parametrize("key", ["esc", "enter", "space"])
def test_confirmation_defaults_to_cancel_and_preserves_branch(branch_confirmation, key):
    app, repo = branch_confirmation
    revision = app.tree()["state"]["status_revision"]
    pyautogui.press(key)
    tree = app.wait_completion(revision)
    assert not find(tree, "test-prompt")
    assert "cancelled" in status(tree).lower()
    assert git(repo, "rev-parse", "--verify", "feature").strip()


@pytest.mark.p0
@pytest.mark.parametrize("key", ["enter", "space"])
def test_keyboard_confirmation_deletes_only_the_selected_merged_branch(branch_confirmation, key):
    app, repo = branch_confirmation
    revision = app.tree()["state"]["status_revision"]
    pyautogui.press("tab")
    pyautogui.press(key)
    tree = app.wait_completion(revision)
    assert not find(tree, "test-prompt")
    assert status(tree) == "Branch operation complete"
    assert git(repo, "show-ref", "--verify", "refs/heads/feature", check=False).returncode != 0
    assert git(repo, "branch", "--show-current").strip() == "main"
