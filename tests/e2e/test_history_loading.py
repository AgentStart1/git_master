"""Branch completeness and stable progressive history through real controls."""
import subprocess

import pytest

from driver import find, git, make_repo, text, walk


@pytest.mark.p1
def test_branch_history_expands_past_200_without_moving_canvas(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    base = git(repo, "rev-parse", "HEAD").strip()
    stream = []
    for branch, start in [("main", 1700100000), ("older", 1700000000)]:
        for index in range(220):
            message = f"{branch} history {index}"
            stream.append(
                f"commit refs/heads/{branch}\n"
                f"committer History Test <history@example.invalid> {start + index} +0000\n"
                f"data {len(message)}\n{message}\n"
                + (f"from {base}\n" if index == 0 else "") + "\n"
            )
    subprocess.run(["git", "fast-import", "--quiet"], cwd=repo,
                   input="".join(stream).encode(), check=True, capture_output=True)
    git(repo, "reset", "--hard", "main")
    newer = git(repo, "rev-parse", "main").strip()
    older = git(repo, "rev-parse", "older").strip()
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-branch-older")
    app.wait(lambda t: app.idle(t) and "older history 219" in text(find(t, "log-list") or {}),
             "older tip survives newer branch's 200 commits")
    app.click("log-view-canvas")
    node_id = f"commit-node-main-{newer}"
    initial = app.wait(lambda t: find(t, node_id) and find(t, node_id).get("bounds"), "initial canvas")
    assert find(initial, f"commit-node-main-{older}")
    app.drag(node_id, 25, 30)
    app.drag("commit-canvas", 15, -20, fraction=(0.9, 0.7))
    width = find(app.tree(), node_id)["bounds"]["width"]
    app.scroll("commit-canvas", 3)
    zoomed = app.wait(lambda t: abs(find(t, node_id)["bounds"]["width"] - width) > 2, "zoom painted")
    before = find(zoomed, node_id)["bounds"]
    app.click("log-load-more")
    expanded = app.wait(lambda t: "All history loaded" in text(find(t, "log-load-more") or {}),
                        "exhausted expanded histories")
    assert find(expanded, node_id)["bounds"] == pytest.approx(before, abs=1)
    assert "main history 0" in text(find(expanded, "commit-canvas"))
    assert "older history 0" in text(find(expanded, "commit-canvas"))
    commits = [node["id"] for node in walk(find(expanded, "commit-canvas"))
               if node.get("node_type") == "commit-node" and node["id"].startswith("commit-node-main-")]
    assert len(commits) == len(set(commits)) == 441
    assert git(repo, "rev-parse", "HEAD").strip() == newer
    assert git(repo, "status", "--porcelain") == ""


@pytest.mark.p1
def test_canvas_automatically_expands_when_panned_near_history_end(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    base = git(repo, "rev-parse", "HEAD").strip()
    stream = []
    for index in range(205):
        message = f"History row {index}"
        stream.append(
            "commit refs/heads/main\n"
            f"committer History Test <history@example.invalid> {1700000000 + index} +0000\n"
            f"data {len(message)}\n{message}\n"
            + (f"from {base}\n" if index == 0 else "") + "\n"
        )
    subprocess.run(["git", "fast-import", "--quiet"], cwd=repo,
                   input="".join(stream).encode(), check=True, capture_output=True)
    git(repo, "reset", "--hard", "main")
    app = app_factory(root)
    app.select(0, repo)
    app.tab("log")
    app.click("log-view-canvas")
    app.scroll("commit-canvas", 5)
    for _ in range(100):
        tree = app.tree()
        if "All history loaded" in text(find(tree, "log-load-more") or {}):
            break
        bounds = find(tree, "commit-canvas")["bounds"]
        # Start and finish inside the canvas so OS pointer clipping cannot
        # truncate the pan or accidentally interact with another application.
        app.drag("commit-canvas", 0, -bounds["height"] * 0.7, fraction=(0.98, 0.9))
    complete = app.wait(lambda t: "All history loaded" in text(find(t, "log-load-more") or {}),
                        "automatic expansion at the canvas frontier")
    assert "History row 0" in text(find(complete, "commit-canvas"))
    assert git(repo, "status", "--porcelain") == ""
