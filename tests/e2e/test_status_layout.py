"""All task details expand above the same anchored status footer."""
import pytest
from driver import find
from test_sync import remote_fixture
from test_task_status import screenshot


def assert_footer_at_bottom(tree):
    footer = find(tree, "status-footer")["bounds"]
    root = find(tree, "root")["bounds"]
    assert abs(footer["y"] + footer["height"] - root["height"]) <= 1
    return footer


@pytest.mark.p1
@pytest.mark.parametrize("kind", ["text", "batch"])
def test_task_details_expand_upward_without_moving_status_summary(app_factory, isolated_git, kind):
    root, repo, _, _ = remote_fixture(isolated_git)
    app = app_factory(root)
    app.select(0, repo)
    if kind == "batch":
        app.click("fetch-all-btn")
        app.wait(lambda t: app.idle(t) and (find(t, "batch-results") or {}).get("bounds"), "batch results expanded")
        app.click("batch-collapse")
    else:
        app.tab("change")
        app.click("worktree-refresh")
    tree = app.wait(lambda t: app.idle(t) and (find(t, "status-footer") or {}).get("bounds"), "collapsed status footer")
    summary_y = find(tree, "status-summary")["bounds"]["y"]
    assert_footer_at_bottom(tree)
    app.click("status-toggle")
    detail_id = "batch-results" if kind == "batch" else "status-details"
    tree = app.wait(lambda t: (find(t, detail_id) or {}).get("bounds"), "details above footer")
    footer = assert_footer_at_bottom(tree)
    details = find(tree, detail_id)["bounds"]
    assert details["y"] + details["height"] <= footer["y"] + 1
    assert abs(find(tree, "status-summary")["bounds"]["y"] - summary_y) <= 1
    screenshot(app, f"{kind}-details-above-footer.png")
    app.click("status-toggle")
    tree = app.wait(lambda t: not (find(t, detail_id) or {}).get("bounds"), "details collapsed")
    assert_footer_at_bottom(tree)
    assert abs(find(tree, "status-summary")["bounds"]["y"] - summary_y) <= 1
