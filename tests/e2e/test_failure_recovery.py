"""Recovery checks require a new result, intact data, and a working next action."""
import pytest
from driver import commit_all, find, git, make_repo, status, text


def branching_repo(isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    git(repo, "checkout", "-b", "feature")
    (repo / "tracked.txt").write_text("feature work\n", encoding="utf-8")
    commit_all(repo, "Unmerged feature history")
    git(repo, "checkout", "main")
    return root, repo


@pytest.mark.p1
def test_failed_branch_delete_retains_visible_history_then_retry_succeeds(app_factory, isolated_git):
    root, repo = branching_repo(isolated_git)
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-branch-feature")
    app.wait(lambda t: status(t) == "Git Log updated" and "Unmerged feature history" in text(find(t, "log-list") or {}))
    app.click("log-delete-feature")
    app.prompt(True)
    assert "not fully merged" in status(app.tree()).lower()
    assert git(repo, "rev-parse", "--verify", "feature").strip()
    assert "Unmerged feature history" in text(find(app.tree(), "log-list")), "failure must retain visible branch history"
    app.click("log-view-canvas")
    tree = app.wait(lambda t: find(t, "commit-canvas"), "history canvas after failure")
    assert "Unmerged feature history" in text(find(tree, "commit-canvas"))
    # Resolve the rejection with an actual merge, then retry the same UI operation.
    git(repo, "merge", "--no-edit", "feature")
    app.click("log-delete-feature")
    app.prompt(True)
    assert status(app.tree()) == "Branch operation complete"
    assert git(repo, "show-ref", "--verify", "refs/heads/feature", check=False).returncode != 0


@pytest.mark.p1
def test_failed_checkout_restores_filter_then_retry_succeeds(app_factory, isolated_git):
    root, repo = branching_repo(isolated_git)
    (repo / "tracked.txt").write_text("unsaved main work\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    assert "Unmerged feature history" not in text(find(app.tree(), "log-list"))
    app.click("log-checkout-feature")
    assert "failed" in status(app.tree()).lower()
    assert git(repo, "branch", "--show-current").strip() == "main"
    assert (repo / "tracked.txt").read_text() == "unsaved main work\n"
    assert "Unmerged feature history" not in text(find(app.tree(), "log-list")), "failed checkout must not add a visible branch"
    git(repo, "stash", "push", "-m", "Preserve local edit before retry")
    app.click("log-checkout-feature")
    assert status(app.tree()) == "Branch operation complete"
    assert git(repo, "branch", "--show-current").strip() == "feature"
    assert "Unmerged feature history" in text(find(app.tree(), "log-list"))
    assert git(repo, "show", "stash@{0}:tracked.txt") == "unsaved main work\n"


@pytest.mark.p0
def test_stash_delete_cancel_has_new_result_and_retry_works(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    (repo / "tracked.txt").write_text("saved work\n", encoding="utf-8")
    git(repo, "stash", "push")
    oid = git(repo, "rev-parse", "refs/stash").strip()
    app = app_factory(root)
    app.select(0, repo)
    app.tab("stash")
    app.click(f"stash-show-{oid}")
    app.click(f"stash-drop-{oid}")
    app.wait(lambda t: find(t, "test-prompt"))
    revision = app.tree()["state"]["status_revision"]
    app.prompt(False)
    assert app.tree()["state"]["status_revision"] > revision, "cancel cannot reuse a previous result"
    assert "cancelled" in status(app.tree()).lower()
    assert git(repo, "rev-parse", "refs/stash").strip() == oid
    app.click(f"stash-drop-{oid}")
    app.prompt(True)
    assert status(app.tree()) == "Git operation complete"
    assert git(repo, "stash", "list") == ""


@pytest.mark.p0
def test_repeated_stash_apply_failures_then_success_do_not_leave_busy_or_stale_error(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    (repo / "tracked.txt").write_text("stash work\n", encoding="utf-8")
    git(repo, "stash", "push")
    oid = git(repo, "rev-parse", "refs/stash").strip()
    (repo / "tracked.txt").write_text("local conflict\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("stash")
    for _ in range(2):
        revision = app.tree()["state"]["status_revision"]
        app.click(f"stash-apply-{oid}")
        assert app.tree()["state"]["status_revision"] > revision
        assert app.idle(app.tree()) and "failed" in status(app.tree()).lower()
        assert (repo / "tracked.txt").read_text() == "local conflict\n"
        assert git(repo, "rev-parse", "refs/stash").strip() == oid
    # The fixture deliberately resolves the local conflict before the final retry.
    git(repo, "checkout", "--", "tracked.txt")
    app.click(f"stash-apply-{oid}")
    assert app.idle(app.tree()) and status(app.tree()) == "Git operation complete"
    assert (repo / "tracked.txt").read_text() == "stash work\n"
    assert git(repo, "rev-parse", "refs/stash").strip() == oid
