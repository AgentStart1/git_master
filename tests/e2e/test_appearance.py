from pathlib import Path
import pytest
from driver import make_repo, git, commit_all, find, text, wait_for


@pytest.mark.p1
def test_theme_switch_keeps_history_filter_and_commit_details(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    git(repo, "checkout", "-b", "feature")
    commit_all(repo, "Feature history")
    git(repo, "checkout", "main")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-branch-feature")
    app.wait(lambda t: app.idle(t) and "Feature history" in text(find(t, "log-list") or {}), "filtered history")
    app.click("log-branch-picker")
    oid = git(repo, "rev-parse", "HEAD").strip()
    app.click(f"log-commit-{oid}")
    app.wait(lambda t: oid in text(find(t, "commit-details") or {}), "commit details")
    branches = app.tree()["state"]["display_branches"]
    for theme in ("light", "dark"):
        app.click(f"theme-{theme}")
        tree = app.wait(lambda t: t["state"]["theme"] == theme, f"{theme} theme")
        assert tree["state"]["display_branches"] == branches
        assert tree["state"]["tab"] == "log"
        assert oid in text(find(tree, "commit-details"))
        assert git(repo, "branch", "--show-current").strip() == "main"
        import pyautogui
        x, y, width, height = app.window_geometry()
        pyautogui.screenshot(region=(x, y, width, height)).save(app.artifacts / f"{theme}.png")


@pytest.mark.p1
def test_theme_preference_survives_restart(app_factory, isolated_git):
    root = isolated_git / "workspace"
    make_repo(root / "project")
    app = app_factory(root)
    app.click("theme-light")
    preference = app.artifacts / "git-config" / "appearance"
    wait_for(lambda: preference.exists() and preference.read_text().strip() == "light", description="saved light theme")
    app.close()
    restarted = app_factory(root)
    assert restarted.tree()["state"]["theme"] == "light"


@pytest.mark.p1
def test_repository_search_and_filter_preserve_selection(app_factory, isolated_git):
    import pyautogui
    root = isolated_git / "workspace"
    alpha = make_repo(root / "alpha")
    beta = make_repo(root / "beta")
    (beta / "local.txt").write_text("local changes")
    app = app_factory(root)
    app.select(0, alpha)
    app.click("repo-filter-modified")
    app.wait(lambda t: not (find(t, "repo-0") or {}).get("bounds") and (find(t, "repo-1") or {}).get("bounds"), "modified filter")
    app.click("repo-filter-all")
    app.click("repo-search")
    # Paste through the native input handler; typing may enter the active OS IME.
    import pyperclip
    pyperclip.copy("beta")
    app.click("repo-search")
    pyautogui.hotkey("ctrl", "v")
    app.wait(lambda t: not (find(t, "repo-0") or {}).get("bounds") and (find(t, "repo-1") or {}).get("bounds"), "search result")
    assert Path(app.tree()["state"]["path"]).resolve() == alpha.resolve()
    pyautogui.hotkey("ctrl", "a")
    pyautogui.press("backspace")
    app.wait(lambda t: (find(t, "repo-0") or {}).get("bounds"), "search cleared")
