import pytest
from driver import commit_all, find, git, make_repo, status, text, wait_for


def remote_fixture(isolated_git):
    remote = isolated_git / "remote.git"
    remote.mkdir()
    git(remote, "init", "--bare", "--initial-branch=main")
    seed = make_repo(isolated_git / "seed")
    git(seed, "remote", "add", "origin", str(remote))
    git(seed, "push", "-u", "origin", "main")
    root = isolated_git / "workspace"
    root.mkdir()
    git(root, "clone", str(remote), "project")
    repo = root / "project"
    git(repo, "config", "user.name", "E2E Tests")
    git(repo, "config", "user.email", "e2e@example.invalid")
    git(repo, "config", "core.autocrlf", "false")
    return root, repo, remote, seed


@pytest.mark.p1
def test_fetch_pull_autostash_and_push_with_local_remote(app_factory, isolated_git):
    root, repo, remote, seed = remote_fixture(isolated_git)
    original = git(repo, "rev-parse", "HEAD").strip()
    (seed / "remote.txt").write_text("remote advance\n", encoding="utf-8")
    commit_all(seed, "Remote advance")
    git(seed, "push", "origin", "main")
    incoming = git(seed, "rev-parse", "HEAD").strip()
    (repo / "tracked.txt").write_text("preserve unstaged local edit\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-remote-origin-fetch")
    wait_for(lambda: git(repo, "rev-parse", "origin/main").strip() == incoming, description="fetched reference")
    app.wait(lambda tree: app.idle(tree) and "Fetch complete" in text(tree), "fetch complete")
    assert git(repo, "rev-parse", "HEAD").strip() == original
    assert "↓1" in text(app.tree())
    app.click("log-remote-origin-pull")
    wait_for(lambda: git(repo, "rev-parse", "HEAD").strip() == incoming, description="pull head")
    app.wait(lambda tree: app.idle(tree) and "Branch operation complete" in text(tree), "pull complete")
    assert (repo / "tracked.txt").read_text(encoding="utf-8") == "preserve unstaged local edit\n"
    assert (repo / "remote.txt").read_text(encoding="utf-8") == "remote advance\n"
    commit_all(repo, "Local outgoing commit")
    outgoing = git(repo, "rev-parse", "HEAD").strip()
    app.select(0, repo)
    app.tab("log")
    app.click("log-remote-origin-push")
    wait_for(lambda: git(remote, "rev-parse", "main").strip() == outgoing, description="pushed remote commit")
    app.wait(lambda tree: app.idle(tree) and "Branch operation complete" in text(tree), "push complete")
    assert "↑0 ↓0" in text(app.tree())


@pytest.mark.p0
@pytest.mark.parametrize("dirty", [False, True])
def test_reset_confirmation_and_dirty_tree_protection(app_factory, isolated_git, dirty):
    root, repo, remote, _ = remote_fixture(isolated_git)
    expected_remote = git(remote, "rev-parse", "main").strip()
    (repo / "outgoing.txt").write_text("local-only commit\n", encoding="utf-8")
    commit_all(repo, "Unpushed commit")
    original = git(repo, "rev-parse", "HEAD").strip()
    if dirty:
        (repo / "tracked.txt").write_text("do not discard this edit\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-remote-origin-reset")
    app.wait(lambda tree: find(tree, "test-prompt"), "reset confirmation")
    assert git(repo, "rev-parse", "HEAD").strip() == original
    app.prompt(False)
    assert git(repo, "rev-parse", "HEAD").strip() == original
    app.click("log-remote-origin-reset")
    app.prompt(True)
    if dirty:
        assert git(repo, "rev-parse", "HEAD").strip() == original
        assert (repo / "tracked.txt").read_text(encoding="utf-8") == "do not discard this edit\n"
        assert (repo / "outgoing.txt").exists()
        assert "failed" in status(app.tree()).lower()
    else:
        assert git(repo, "rev-parse", "HEAD").strip() == expected_remote
        assert not (repo / "outgoing.txt").exists()
        assert git(repo, "status", "--porcelain") == ""


@pytest.mark.p1
def test_fetch_all_continues_after_remote_failure(app_factory, isolated_git):
    root, repo, remote, seed = remote_fixture(isolated_git)
    git(repo, "remote", "add", "a-broken", str(isolated_git / "missing.git"))
    second = make_repo(root / "second")
    git(second, "remote", "add", "origin", str(remote))
    (seed / "fetched.txt").write_text("batch advance\n", encoding="utf-8")
    commit_all(seed, "Batch remote update")
    git(seed, "push", "origin", "main")
    expected = git(seed, "rev-parse", "HEAD").strip()
    (repo / "local.txt").write_text("keep local untracked file\n", encoding="utf-8")
    app = app_factory(root)
    app.click("fetch-all-btn")
    app.wait(lambda tree: app.idle(tree) and "failed" in status(tree).lower(), "batch completion with reported failure")
    for path in (repo, second):
        assert git(path, "rev-parse", "origin/main").strip() == expected
    assert (repo / "local.txt").read_text(encoding="utf-8") == "keep local untracked file\n"


@pytest.mark.p1
def test_switch_all_to_main_updates_each_repository(app_factory, isolated_git):
    root = isolated_git / "workspace"
    first, second = make_repo(root / "first"), make_repo(root / "second")
    for repo in (first, second):
        git(repo, "checkout", "-b", "dev")
    (second / "untracked.txt").write_text("keep local\n", encoding="utf-8")
    app = app_factory(root)
    app.click("switch-all-main-btn")
    app.wait(lambda tree: app.idle(tree) and "succeeded" in text(tree), "switch all completion")
    for repo in (first, second):
        assert git(repo, "branch", "--show-current").strip() == "main"
    assert (second / "untracked.txt").read_text(encoding="utf-8") == "keep local\n"
