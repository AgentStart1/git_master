"""Real input coverage for the overview, branch keyboard menu and scoped batch retry."""
import pytest
from driver import make_repo, git, text, find, commit_all

@pytest.mark.p1
def test_overview_links_and_branch_keyboard_filter(app_factory, isolated_git):
    import pyautogui
    root=isolated_git / "workspace"
    repo=make_repo(root / "project")
    git(repo,"branch","feature")
    app=app_factory(root)
    app.select(0,repo)
    original=git(repo,"rev-parse","HEAD")
    app.tab("info")
    app.click("overview-changes")
    app.wait(lambda t:app.idle(t) and t["state"]["tab"]=="change","overview changes")
    app.tab("info")
    app.click("overview-history")
    app.click("log-branch-picker")
    pyautogui.press("tab")
    pyautogui.press("space")
    app.wait(lambda t:app.idle(t) and "feature" in t["state"]["display_branches"],"keyboard branch selection")
    pyautogui.press("esc")
    app.wait(lambda t:not (find(t,"log-branch-feature") or {}).get("bounds"),"keyboard menu dismissal")
    assert git(repo,"rev-parse","HEAD")==original
    assert git(repo,"branch","--show-current").strip()=="main"

@pytest.mark.p0
def test_batch_retry_only_runs_failed_remote(app_factory, isolated_git):
    root=isolated_git / "workspace"
    repo=make_repo(root / "project")
    source=isolated_git / "source.git"
    git(repo,"init","--bare",str(source))
    missing=isolated_git / "missing.git"
    git(repo,"remote","add","good",str(source))
    git(repo,"remote","add","retry",str(missing))
    app=app_factory(root)
    app.select(0,repo)
    app.click("fetch-all-btn")
    app.wait(lambda t:"1 succeeded, 1 failed" in text(t),"partial batch result")
    git(repo,"remote","set-url","good",str(isolated_git / "must-not-fetch.git"))
    git(repo,"init","--bare",str(missing))
    app.click("batch-retry-failed",await_operation=False)
    app.wait(lambda t:app.idle(t) and "1 succeeded, 0 failed" in text(t),"failed remote retried alone")
    assert "must-not-fetch" in git(repo,"remote","get-url","good")

@pytest.mark.p0
def test_batch_switch_cancel_leaves_all_heads_unchanged(app_factory, isolated_git):
    root=isolated_git / "workspace"
    repo=make_repo(root / "project")
    git(repo,"checkout","-b","feature")
    app=app_factory(root)
    app.select(0,repo)
    app.click("switch-all-main-btn",await_operation=False)
    app.prompt(False)
    assert git(repo,"branch","--show-current").strip()=="feature"

@pytest.mark.p0
def test_conflict_preview_and_cancel_preserve_unmerged_index(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    git(repo, "checkout", "-b", "topic")
    (repo / "tracked.txt").write_text("topic content\n")
    commit_all(repo, "Topic")
    git(repo, "checkout", "main")
    (repo / "tracked.txt").write_text("main content\n")
    commit_all(repo, "Main")
    git(repo, "merge", "topic", check=False)
    before_index = git(repo, "ls-files", "-u")
    before_file = (repo / "tracked.txt").read_bytes()
    assert before_index
    app = app_factory(root)
    app.select(0, repo)
    app.tab("change")
    app.change(repo, "tracked.txt", "diff")
    preview = text(find(app.tree(), "worktree-diff"))
    assert "<<<<<<<" in preview and "topic content" in preview and "main content" in preview
    app.click("change-stage-0", await_operation=False)
    app.prompt(False)
    assert git(repo, "ls-files", "-u") == before_index
    assert (repo / "tracked.txt").read_bytes() == before_file
