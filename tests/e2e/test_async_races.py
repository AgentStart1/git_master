"""Hold real Git operations at hooks while navigating the actual desktop UI."""
from pathlib import Path
import shutil
import pytest
from driver import commit_all, find, git, make_repo, status, text, wait_for
from test_sync import remote_fixture


@pytest.fixture
def gate(isolated_git, monkeypatch):
    folder = isolated_git / "gate"
    folder.mkdir()
    monkeypatch.setenv("GIT_MASTER_E2E_GATE", str(folder))
    class Gate:
        def install(self, repo, hook):
            self.install_git_dir(repo / ".git", hook)

        def install_git_dir(self, git_dir, hook):
            target = git_dir / "hooks" / hook
            shutil.copyfile(Path(__file__).parent / "fixtures" / "block_hook.sh", target)
            target.chmod(0o755)

        def entered(self):
            wait_for(lambda: (folder / "entered").exists(), description="Git hook reached")

        def release(self):
            (folder / "release").touch()
    value = Gate()
    yield value
    value.release()


def select_while_busy(app, index, path):
    # The status strip changes row positions when a mutation starts. Resolve
    # coordinates only after that busy frame has actually been painted.
    app.wait(lambda t: t["state"]["busy"] and find(t, "status-summary") and
        find(t, "status-summary").get("bounds"), "mutation progress painted before navigation")
    app.click(f"repo-{index}")
    app.wait(lambda t: not t["state"]["loading_detail"] and t["state"].get("path") and
        Path(t["state"]["path"]).resolve() == path.resolve(), "new repository loaded during operation")
    assert app.tree()["state"]["busy"], "hook must still hold the old operation"


@pytest.mark.p1
@pytest.mark.parametrize("tab", ["change", "stash"])
def test_other_repository_can_load_changes_while_push_is_pending(app_factory, isolated_git, gate, tab):
    root, repo, remote, _ = remote_fixture(isolated_git)
    other = make_repo(root / "second")
    (other / "other-only.txt").write_text("other repository data\n", encoding="utf-8")
    if tab == "stash":
        git(other, "stash", "push", "--include-untracked", "-m", "Other-only saved work")
    (repo / "outgoing.txt").write_text("push from original repository\n", encoding="utf-8")
    commit_all(repo, "Outgoing during navigation")
    outgoing = git(repo, "rev-parse", "HEAD").strip()
    gate.install(repo, "pre-push")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    revision = app.tree()["state"]["status_revision"]
    app.click("log-remote-origin-push", await_operation=False)
    gate.entered()
    select_while_busy(app, 1, other)
    app.click(f"tab-{tab}")
    expected = "other-only.txt" if tab == "change" else "Other-only saved work"
    app.wait(lambda t: not t["state"]["worktree_loading"] and expected in text(find(t, "worktree-content") or {}),
        "new repository tab loads before old push completes")
    assert app.tree()["state"]["busy"]
    assert text(find(app.tree(), "status-summary")) == "Pushing main to origin…"
    gate.release()
    app.wait_completion(revision)
    assert Path(app.tree()["state"]["path"]).resolve() == other.resolve()
    assert expected in text(find(app.tree(), "worktree-content"))
    assert "outgoing.txt" not in text(find(app.tree(), "worktree-content"))
    assert git(remote, "rev-parse", "main").strip() == outgoing


@pytest.mark.p1
def test_old_branch_deletion_does_not_rewrite_new_repository_filter(app_factory, isolated_git, gate):
    root = isolated_git / "workspace"
    first, other = make_repo(root / "alpha"), make_repo(root / "beta")
    git(first, "branch", "feature")
    git(other, "checkout", "-b", "feature")
    commit_all(other, "Other feature history")
    git(other, "checkout", "main")
    commit_all(other, "Other main history")
    git(other, "checkout", "feature")
    gate.install(first, "reference-transaction")
    app = app_factory(root)
    app.select(0, first)
    app.tab("log")
    app.click("log-delete-feature")
    app.prompt(True, await_operation=False)
    gate.entered()
    revision = app.tree()["state"]["status_revision"]
    select_while_busy(app, 1, other)
    app.click("tab-log")
    app.wait(lambda t: find(t, "log-list"), "other history")
    gate.release()
    app.wait_completion(revision)
    assert git(first, "show-ref", "--verify", "refs/heads/feature", check=False).returncode != 0
    assert git(other, "branch", "--show-current").strip() == "feature"
    previous = app.tree()["state"]["status_revision"]
    app.click("log-branch-main")
    app.wait(lambda t: t["state"]["status_revision"] > previous and status(t) == "Git Log updated", "hide main in other repository")
    contents = text(find(app.tree(), "log-list"))
    assert "Other feature history" in contents
    assert "Other main history" not in contents


@pytest.mark.p1
def test_returning_to_original_repo_does_not_restore_old_filter_snapshot(app_factory, isolated_git, gate):
    root, repo, _, _ = remote_fixture(isolated_git)
    other = make_repo(root / "second")
    git(repo, "checkout", "-b", "feature")
    commit_all(repo, "Feature-only prior view")
    git(repo, "checkout", "main")
    commit_all(repo, "Main-only current view")
    gate.install(repo, "pre-push")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-branch-feature")
    app.wait(lambda t: status(t) == "Git Log updated" and "Feature-only prior view" in text(find(t, "log-list") or {}))
    app.click("log-branch-main")
    app.wait(lambda t: status(t) == "Git Log updated" and "Main-only current view" not in text(find(t, "log-list") or {}))
    app.click("log-remote-origin-push", await_operation=False)
    gate.entered()
    revision = app.tree()["state"]["status_revision"]
    select_while_busy(app, 1, other)
    select_while_busy(app, 0, repo)
    app.click("tab-log")
    app.wait(lambda t: "Main-only current view" in text(find(t, "log-list") or {}), "new main history after returning")
    gate.release()
    app.wait_completion(revision)
    contents = text(find(app.tree(), "log-list"))
    assert "Main-only current view" in contents
    assert "Feature-only prior view" not in contents


@pytest.mark.p1
def test_submodule_init_completion_does_not_open_other_parents_same_relative_path(app_factory, isolated_git, gate):
    source = make_repo(isolated_git / "source")
    root = isolated_git / "workspace"
    first, other = make_repo(root / "alpha"), make_repo(root / "beta")
    for repo in (first, other):
        git(repo, "submodule", "add", str(source), "module")
        commit_all(repo, "Add same module path")
        git(repo, "submodule", "deinit", "-f", "module")
    gate.install_git_dir(first / ".git" / "modules" / "module", "post-checkout")
    app = app_factory(root)
    app.click("repo-0-toggle")
    app.click("repo-0-submodule-0")
    app.wait(lambda t: find(t, "submodule-info-content"), "first uninitialized module")
    app.click("init-submodule-btn", await_operation=False)
    gate.entered()
    app.wait(lambda t: t["state"]["busy"] and "Initializing" in status(t), "initialization busy frame painted")
    revision = app.tree()["state"]["status_revision"]
    app.click("repo-1-toggle")
    app.click("repo-1-submodule-0")
    app.wait(lambda t: "Not initialized" in text(find(t, "submodule-info-content") or {}), "other uninitialized module")
    assert app.tree()["state"]["path"] is None
    assert app.tree()["state"]["busy"]
    gate.release()
    app.wait_completion(revision)
    assert "Not initialized" in text(find(app.tree(), "submodule-info-content") or {})
    assert app.tree()["state"]["path"] is None, "an uninitialized module must not acquire its parent's repository details"
    assert not (other / "module" / ".git").exists()
    assert (first / "module" / ".git").is_file()
