"""Workspace menu actions survive a normal mouse press/release interval."""
import pytest
import sys
from driver import find, git, make_repo, text, wait_for
from test_workspace_dialog import active_folder_dialog


@pytest.mark.p1
def test_reopen_current_workspace_refreshes_selected_repository(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("info")
    git(repo, "checkout", "-b", "external-branch")
    app.click("workspace-menu", await_operation=False)
    app.click("recent-workspace-0", await_operation=False)
    tree = app.wait(
        lambda t: app.idle(t) and "Branch: external-branch" in text(find(t, "info-content")),
        "reopened workspace detail refresh",
    )
    assert "Branch: external-branch" in text(find(tree, "repo-0"))


@pytest.mark.p1
@pytest.mark.skipif(sys.platform != "win32", reason="Includes native Windows directory picker")
@pytest.mark.parametrize("action", ["change-dir-btn", "open-logs-btn"])
def test_workspace_menu_action_survives_mouse_press(app_factory, isolated_git, action):
    import pyautogui
    root = isolated_git / "workspace"
    make_repo(root / "project")
    app = app_factory(root)
    app.click("workspace-menu")
    position = app.point(action)
    before = app.tree()["state"]["view_revision"]
    pyautogui.mouseDown(*position)
    try:
        # Let GPUI publish a fresh frame while the mouse is held, as during a human click.
        app.wait(lambda t: t["state"]["view_revision"] > before, "frame during held menu press")
        assert (app.node(action) or {}).get("bounds"), "menu dismissed before click release"
    finally:
        pyautogui.mouseUp()
    if action == "change-dir-btn":
        wait_for(lambda: active_folder_dialog(app), description="native directory picker opened")
        pyautogui.press("esc")
    else:
        app.wait(lambda _: find(app.rpc("get_logs_view_tree") or {}, "logs-root"), "logs window opened")


@pytest.mark.p1
@pytest.mark.parametrize("target", ["workspace-menu", "repo-0"])
def test_workspace_menu_dismisses_on_trigger_or_outside_click(app_factory, isolated_git, target):
    root = isolated_git / "workspace"
    make_repo(root / "project")
    app = app_factory(root)
    app.click("workspace-menu")
    app.wait(lambda t: (find(t, "change-dir-btn") or {}).get("bounds"), "workspace menu open")
    app.click(target)
    app.wait(lambda t: not (find(t, "change-dir-btn") or {}).get("bounds"), "workspace menu dismissed")
