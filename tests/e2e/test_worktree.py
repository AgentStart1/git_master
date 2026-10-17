from pathlib import Path
import pytest
from driver import commit_all, find, git, make_repo, status, text, wait_for, walk


def worktree(app_factory, isolated_git, *, commit=True):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project", commit=commit)
    return root, repo


@pytest.mark.p0
@pytest.mark.parametrize("kind", ["modified", "new", "deleted", "literal", "unborn"])
def test_stage_unstage_preserves_working_files(app_factory, isolated_git, kind):
    root, repo = worktree(app_factory, isolated_git, commit=kind != "unborn")
    relative = "file[1].txt" if kind == "literal" else "new.txt" if kind in ("new", "unborn") else "tracked.txt"
    path = repo / relative
    if kind == "deleted":
        path.unlink()
    else:
        path.write_text("local edit 中文\n", encoding="utf-8")
    if kind == "literal":
        (repo / "file1.txt").write_text("must remain untracked\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("change")
    app.change(repo, relative, "stage")
    wait_for(lambda: relative in git(repo, "diff", "--cached", "--name-only"), description="index staged file")
    app.wait(lambda tree: app.idle(tree) and any(node.get("id", "").startswith("change-unstage-") for node in walk(tree)), "staged state")
    if kind == "literal":
        assert git(repo, "diff", "--cached", "--name-only").splitlines() == [relative]
    if kind == "modified":
        path.write_text("newest unstaged edit\n", encoding="utf-8")
    expected = path.read_bytes() if path.exists() else None
    app.change(repo, relative, "unstage")
    wait_for(lambda: git(repo, "diff", "--cached", "--name-only") == "", description="empty index")
    app.wait(lambda tree: app.idle(tree) and "Working tree" in text(tree) or app.idle(tree) and "Untracked" in text(tree), "unstaged state")
    assert (path.read_bytes() if path.exists() else None) == expected
    assert "●" in text(find(app.tree(), "repo-0"))


@pytest.mark.p1
def test_diff_separates_staged_and_worktree_and_has_word_ranges(app_factory, isolated_git):
    root, repo = worktree(app_factory, isolated_git)
    path = repo / "tracked.txt"
    path.write_text("staged source\n", encoding="utf-8")
    git(repo, "add", "tracked.txt")
    path.write_text("working source\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("change")
    app.change(repo, "tracked.txt", "diff")
    tree = app.wait(lambda tree: app.idle(tree) and "working source" in text(find(tree, "worktree-diff") or {}), "diff contents")
    preview = find(tree, "worktree-diff")
    assert "Staged" in text(preview) and "Working tree" in text(preview)
    assert "staged source" in text(preview)
    assert any(row["node_type"] == "diff-added" and row.get("state", {}).get("new_line") == 1 and row.get("state", {}).get("highlights") for row in walk(preview))
    assert any(row["node_type"] == "diff-removed" for row in walk(preview))
    # Refreshing/previewing must never change index or files.
    assert "staged source" in git(repo, "show", ":tracked.txt")
    assert path.read_text(encoding="utf-8") == "working source\n"


@pytest.mark.p0
@pytest.mark.parametrize("operation", ["apply", "pop"])
def test_save_stash_preview_and_restore(app_factory, isolated_git, operation):
    root, repo = worktree(app_factory, isolated_git)
    path = repo / "tracked.txt"
    path.write_text("staged stash version\n", encoding="utf-8")
    git(repo, "add", "tracked.txt")
    path.write_text("working stash version\n", encoding="utf-8")
    (repo / "new.txt").write_text("saved untracked content\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("stash")
    app.click("stash-save")
    oid = wait_for(lambda: git(repo, "rev-parse", "--verify", "refs/stash", check=False).stdout.strip(), description="saved stash")
    app.wait(lambda tree: app.idle(tree) and find(tree, f"stash-show-{oid}"), "stash list")
    assert git(repo, "status", "--porcelain") == ""
    app.click(f"stash-show-{oid}")
    tree = app.wait(lambda tree: app.idle(tree) and "saved untracked content" in text(find(tree, "worktree-diff") or {}), "stash preview")
    for label in ("Staged changes", "Working tree changes", "Untracked files", "staged stash version", "working stash version"):
        assert label in text(find(tree, "worktree-diff"))
    app.click(f"stash-{operation}-{oid}")
    wait_for(lambda: path.read_text(encoding="utf-8") == "working stash version\n", description="restored tracked file")
    app.wait(lambda tree: app.idle(tree) and "Git operation complete" in text(tree), "stash operation finished")
    assert (repo / "new.txt").read_text(encoding="utf-8") == "saved untracked content\n"
    assert bool(git(repo, "stash", "list")) == (operation == "apply")
    assert "●" in text(find(app.tree(), "repo-0"))


@pytest.mark.p0
def test_stash_delete_requires_confirmation_and_preserves_files(app_factory, isolated_git):
    root, repo = worktree(app_factory, isolated_git)
    (repo / "tracked.txt").write_text("saved for deletion\n", encoding="utf-8")
    git(repo, "stash", "push", "-m", "Delete candidate")
    oid = git(repo, "rev-parse", "refs/stash").strip()
    (repo / "tracked.txt").write_text("keep local work\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("stash")
    app.click(f"stash-drop-{oid}")
    app.wait(lambda tree: find(tree, "test-prompt"), "delete prompt")
    assert git(repo, "rev-parse", "refs/stash").strip() == oid
    app.prompt(False)
    assert git(repo, "rev-parse", "refs/stash").strip() == oid
    app.click(f"stash-drop-{oid}")
    app.prompt(True)
    assert git(repo, "stash", "list") == ""
    assert (repo / "tracked.txt").read_text(encoding="utf-8") == "keep local work\n"


@pytest.mark.p0
@pytest.mark.parametrize("operation", ["apply", "pop"])
def test_conflicting_stash_restore_keeps_stash_and_local_edits(app_factory, isolated_git, operation):
    root, repo = worktree(app_factory, isolated_git)
    (repo / "tracked.txt").write_text("stash change\n", encoding="utf-8")
    git(repo, "stash", "push", "-m", "Conflicting save")
    oid = git(repo, "rev-parse", "refs/stash").strip()
    (repo / "tracked.txt").write_text("precious local edit\n", encoding="utf-8")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("stash")
    app.click(f"stash-{operation}-{oid}")
    app.wait(lambda tree: app.idle(tree) and "failed" in status(tree).lower(), "safe restore failure")
    assert git(repo, "rev-parse", "refs/stash").strip() == oid
    assert (repo / "tracked.txt").read_text(encoding="utf-8") == "precious local edit\n"
