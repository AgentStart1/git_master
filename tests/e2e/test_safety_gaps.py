"""Failure paths and identity safeguards missing from the initial workflow suite."""
import pytest
from pathlib import Path
from driver import commit_all, find, git, make_repo, status, text, wait_for
from test_sync import remote_fixture


@pytest.mark.p0
def test_rejected_push_preserves_both_divergent_histories(app_factory, isolated_git):
    root, repo, remote, seed = remote_fixture(isolated_git)
    (seed / "remote.txt").write_text("remote-only\n", encoding="utf-8")
    commit_all(seed, "Remote-only commit")
    git(seed, "push", "origin", "main")
    remote_head = git(remote, "rev-parse", "main").strip()
    (repo / "local.txt").write_text("local-only\n", encoding="utf-8")
    commit_all(repo, "Local-only commit")
    local_head = git(repo, "rev-parse", "HEAD").strip()
    (repo / "tracked.txt").write_text("unsaved work\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-remote-origin-push")
    app.wait(lambda t: app.idle(t) and "failed" in status(t).lower(), "non-fast-forward rejection")
    assert git(repo, "rev-parse", "HEAD").strip() == local_head
    assert git(remote, "rev-parse", "main").strip() == remote_head
    assert (repo / "tracked.txt").read_text() == "unsaved work\n"


@pytest.mark.p0
def test_pull_conflict_retains_commits_and_recoverable_local_work(app_factory, isolated_git):
    root, repo, remote, seed = remote_fixture(isolated_git)
    for path, content in ((seed, "remote version\n"), (repo, "local version\n")):
        (path / "tracked.txt").write_text(content, encoding="utf-8")
        commit_all(path, content.strip())
    git(seed, "push", "origin", "main")
    local_head = git(repo, "rev-parse", "HEAD").strip()
    remote_head = git(remote, "rev-parse", "main").strip()
    (repo / "untracked.txt").write_text("precious untracked\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-remote-origin-pull")
    app.wait(lambda t: app.idle(t) and "failed" in status(t).lower(), "rebase conflict reported")
    assert "tracked.txt" in git(repo, "diff", "--name-only", "--diff-filter=U")
    assert git(repo, "show", f"{local_head}:tracked.txt") == "local version\n"
    assert git(remote, "rev-parse", "main").strip() == remote_head
    assert (repo / "untracked.txt").read_text() == "precious untracked\n"
    # Recovery is fixture cleanup, not an application action under test.
    git(repo, "rebase", "--abort")
    assert git(repo, "rev-parse", "HEAD").strip() == local_head


@pytest.mark.p0
@pytest.mark.parametrize("action", ["apply", "pop", "drop"])
def test_stash_operation_targets_selected_oid_after_index_shift(app_factory, isolated_git, action):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    ids = []
    for label in ("older", "newer"):
        (repo / "tracked.txt").write_text(f"{label} saved work\n", encoding="utf-8")
        git(repo, "stash", "push", "-m", label)
        ids.append(git(repo, "rev-parse", "refs/stash").strip())
    older, newer = ids
    app = app_factory(root)
    app.select(0, repo)
    app.tab("stash")
    # Change stash indexes behind the UI. Actions must resolve identity, not index.
    git(repo, "stash", "drop", "stash@{0}")
    app.click(f"stash-{action}-{older}")
    if action == "drop":
        app.prompt(True)
    app.wait(lambda t: app.idle(t) and "Git operation complete" in text(t), "selected stash operation")
    expected = "base\n" if action == "drop" else "older saved work\n"
    assert (repo / "tracked.txt").read_text() == expected
    remaining = git(repo, "stash", "list", "--format=%H").splitlines()
    assert remaining == ([older] if action == "apply" else [])
    assert newer not in remaining


@pytest.mark.p0
def test_removed_stash_cannot_act_on_replacement(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    (repo / "tracked.txt").write_text("stale stash\n", encoding="utf-8")
    git(repo, "stash", "push")
    stale = git(repo, "rev-parse", "refs/stash").strip()
    app = app_factory(root)
    app.select(0, repo)
    app.tab("stash")
    git(repo, "stash", "drop")
    (repo / "tracked.txt").write_text("replacement stash\n", encoding="utf-8")
    git(repo, "stash", "push")
    replacement = git(repo, "rev-parse", "refs/stash").strip()
    app.click(f"stash-pop-{stale}")
    app.wait(lambda t: app.idle(t) and "failed" in status(t).lower(), "stale identity rejected")
    assert git(repo, "rev-parse", "refs/stash").strip() == replacement
    assert (repo / "tracked.txt").read_text() == "base\n"


@pytest.mark.p0
def test_failed_stash_save_without_identity_preserves_index_and_files(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    git(repo, "config", "--unset", "user.name")
    git(repo, "config", "--unset", "user.email")
    # Prevent Git/libgit2 from guessing the machine identity.
    git(repo, "config", "user.useConfigOnly", "true")
    (repo / "tracked.txt").write_text("staged work\n", encoding="utf-8")
    git(repo, "add", "tracked.txt")
    (repo / "tracked.txt").write_text("working work\n", encoding="utf-8")
    before = git(repo, "status", "--porcelain")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("stash")
    app.click("stash-save")
    app.wait(lambda t: app.idle(t) and "failed" in status(t).lower(), "identity failure")
    assert git(repo, "status", "--porcelain") == before
    assert git(repo, "show", ":tracked.txt") == "staged work\n"
    assert (repo / "tracked.txt").read_text() == "working work\n"
    assert git(repo, "stash", "list") == ""


@pytest.mark.p0
def test_batch_switch_continues_after_dirty_checkout_failure(app_factory, isolated_git):
    root = isolated_git / "workspace"
    blocked, healthy = make_repo(root / "blocked"), make_repo(root / "healthy")
    for repo in (blocked, healthy):
        git(repo, "checkout", "-b", "feature")
    (blocked / "tracked.txt").write_text("feature committed\n", encoding="utf-8")
    commit_all(blocked, "Feature changes")
    original = git(blocked, "rev-parse", "HEAD").strip()
    (blocked / "tracked.txt").write_text("do not lose dirty work\n", encoding="utf-8")
    app = app_factory(root)
    app.click("switch-all-main-btn")
    app.wait(lambda t: app.idle(t) and "1 succeeded, 1 failed" in text(t), "partial batch completion")
    assert git(blocked, "rev-parse", "HEAD").strip() == original
    assert git(blocked, "branch", "--show-current").strip() == "feature"
    assert (blocked / "tracked.txt").read_text() == "do not lose dirty work\n"
    assert git(healthy, "branch", "--show-current").strip() == "main"


@pytest.mark.p0
def test_failed_stage_index_lock_is_reported_and_retry_succeeds(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    (repo / "tracked.txt").write_text("keep working bytes\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("change")
    lock = repo / ".git" / "index.lock"
    lock.write_text("owned by another Git operation", encoding="utf-8")
    app.change(repo, "tracked.txt", "stage")
    app.wait(lambda t: app.idle(t) and "failed" in status(t).lower(), "index lock failure")
    assert git(repo, "diff", "--cached", "--name-only") == ""
    assert lock.read_text() == "owned by another Git operation"
    assert (repo / "tracked.txt").read_text() == "keep working bytes\n"
    lock.unlink()
    app.change(repo, "tracked.txt", "stage")
    wait_for(lambda: git(repo, "diff", "--cached", "--name-only").strip() == "tracked.txt")
    app.wait(lambda t: app.idle(t), "retry completed")


@pytest.mark.p0
def test_selected_submodule_stash_does_not_touch_parent_work(app_factory, isolated_git):
    source = make_repo(isolated_git / "source")
    root = isolated_git / "workspace"
    parent = make_repo(root / "parent")
    git(parent, "submodule", "add", str(source), "module")
    commit_all(parent, "Add module")
    module = parent / "module"
    git(module, "config", "user.name", "E2E Tests")
    git(module, "config", "user.email", "e2e@example.invalid")
    (parent / "tracked.txt").write_text("keep parent dirty\n", encoding="utf-8")
    (module / "tracked.txt").write_text("save module work\n", encoding="utf-8")
    app = app_factory(root)
    app.click("repo-0-toggle")
    app.click("repo-0-submodule-0")
    app.wait(lambda t: app.idle(t) and t["state"].get("path") and
        Path(t["state"]["path"]).resolve() == module.resolve(), "selected module")
    app.tab("stash")
    app.click("stash-save")
    app.wait(lambda t: app.idle(t) and "Git operation complete" in status(t), "module stash save")
    oid = git(module, "rev-parse", "refs/stash").strip()
    assert git(parent, "stash", "list") == ""
    assert (parent / "tracked.txt").read_text() == "keep parent dirty\n"
    app.click(f"stash-show-{oid}")
    app.wait(lambda t: app.idle(t) and "save module work" in text(find(t, "worktree-diff") or {}), "module preview")
    app.click(f"stash-pop-{oid}")
    app.wait(lambda t: app.idle(t) and "Git operation complete" in status(t), "module stash pop")
    assert (module / "tracked.txt").read_text() == "save module work\n"
    assert (parent / "tracked.txt").read_text() == "keep parent dirty\n"
    assert git(module, "stash", "list") == git(parent, "stash", "list") == ""


@pytest.mark.p0
def test_reset_missing_remote_branch_does_not_discard_local_commit(app_factory, isolated_git):
    root, repo, _, _ = remote_fixture(isolated_git)
    git(repo, "checkout", "-b", "local-only")
    commit_all(repo, "Keep unpublished commit")
    original = git(repo, "rev-parse", "HEAD").strip()
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-remote-origin-reset")
    app.prompt(True)
    assert "failed" in status(app.tree()).lower()
    assert git(repo, "rev-parse", "HEAD").strip() == original
    assert git(repo, "branch", "--show-current").strip() == "local-only"
