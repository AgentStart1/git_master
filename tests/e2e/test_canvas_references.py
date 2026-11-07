"""Canvas references connect independently draggable labels and commits."""
import pytest
from driver import make_repo, git, commit_all, find, text, walk


@pytest.mark.p1
def test_remote_reference_cards_have_connections_that_follow_dragging(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    base = git(repo, "rev-parse", "HEAD").strip()
    git(repo, "checkout", "-b", "feature")
    commit_all(repo, "Remote feature tip")
    remote_tip = git(repo, "rev-parse", "HEAD").strip()
    commit_all(repo, "Local unpushed change")
    local_tip = git(repo, "rev-parse", "HEAD").strip()
    git(repo, "remote", "add", "origin", "https://example.invalid/repo.git")
    git(repo, "remote", "add", "backup", "https://example.invalid/backup.git")
    for ref, oid in [("origin/main", base), ("backup/main", base), ("origin/feature", remote_tip)]:
        git(repo, "update-ref", f"refs/remotes/{ref}", oid)
    git(repo, "symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-view-canvas")
    canvas = find(app.wait(lambda t: find(t, "commit-canvas"), "canvas"), "commit-canvas")
    references = [n for n in walk(canvas) if n.get("state", {}).get("kind") == "reference" and n["node_type"] == "commit-node"]
    for label in ["HEAD", "feature", "origin/feature", "origin/main", "origin/HEAD", "backup/main"]:
        assert any(label in text(n).splitlines() for n in references), f"missing {label}"
    edges = [n["state"] for n in walk(canvas) if n["node_type"] == "graph-edge" and n.get("state", {}).get("kind") == "reference"]
    for oid in [local_tip, remote_tip, base]:
        assert any(e["from_oid"] == f"head:{oid}" and e["to_oid"] == oid and len(e["route"]) >= 2 for e in edges)
    import pyautogui
    x, y, width, height = app.window_geometry()
    pyautogui.screenshot(region=(x, y, width, height)).save(app.artifacts / "canvas-references.png")
    ref_id = f"commit-node-heads-head:{local_tip}"
    route_before = next(e["route"] for e in edges if e["to_oid"] == local_tip)
    app.drag(ref_id, 35, 20)
    def moved(tree):
        c = find(tree, "commit-canvas") or {}
        return any(n.get("state", {}).get("kind") == "reference" and n["node_type"] == "graph-edge" and n["state"]["to_oid"] == local_tip and n["state"]["route"] != route_before for n in walk(c))
    app.wait(moved, "reference line follows moved card")
    assert not find(app.tree(), "commit-details"), "dragging a reference must not open details"
    assert git(repo, "rev-parse", "HEAD").strip() == local_tip


@pytest.mark.p1
def test_head_is_centered_and_stays_fixed_when_commit_is_dragged(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    oid = git(repo, "rev-parse", "HEAD").strip()
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-view-canvas")
    tree = app.wait(lambda t: find(t, "commit-canvas"), "canvas")
    head_id = f"commit-node-heads-head:{oid}"
    commit_id = f"commit-node-main-{oid}"
    head = find(tree, head_id)["state"]
    commit = find(tree, commit_id)["state"]

    def route(tree):
        return next(n["state"]["route"] for n in walk(find(tree, "commit-canvas"))
                    if n["node_type"] == "graph-edge"
                    and n.get("state", {}).get("kind") == "reference"
                    and n["state"]["to_oid"] == oid)

    center_y = commit["world_y"] + commit["height"] / 2
    assert head["world_y"] + head["height"] / 2 == pytest.approx(center_y)
    assert head["world_x"] == pytest.approx(commit["world_x"] + commit["width"] + 12)
    before = route(tree)
    assert before == [[head["world_x"], center_y], [commit["world_x"] + commit["width"], center_y]]
    app.drag(commit_id, -30, 60)
    tree = app.wait(lambda t: route(t) != before, "reference line follows commit drag")
    moved_head = find(tree, head_id)["state"]
    moved_commit = find(tree, commit_id)["state"]
    assert (moved_head["world_x"], moved_head["world_y"]) == (head["world_x"], head["world_y"])
    assert moved_commit["world_x"] < commit["world_x"]
    assert moved_commit["world_y"] > commit["world_y"]
    assert route(tree)[0] == before[0]
    assert route(tree)[-1] == [moved_commit["world_x"] + moved_commit["width"],
                               moved_commit["world_y"] + moved_commit["height"] / 2]
    assert not find(tree, "commit-details"), "dragging a commit must not open details"
    import pyautogui
    x, y, width, height = app.window_geometry()
    pyautogui.screenshot(region=(x, y, width, height)).save(app.artifacts / "independent-head-after-drag.png")
