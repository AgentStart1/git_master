"""Focus rings follow input modality without removing keyboard focus."""
import math
import pytest
from driver import find, make_repo, wait_for
from test_logs import open_logs


def top_border_pixels(app, node_id, accent):
    import pyautogui
    x, y, width, height = app.window_geometry()
    tree = app.tree()
    bounds = find(tree, node_id)["bounds"]
    root = (find(tree, "root") or find(tree, "logs-root"))["bounds"]
    sx, sy = width / root["width"], height / root["height"]
    left = x + math.floor(bounds["x"] * sx)
    top = y + math.floor(bounds["y"] * sy)
    right = x + math.ceil((bounds["x"] + bounds["width"]) * sx)
    image = pyautogui.screenshot(region=(left, top, right-left, 3)).convert("RGB")
    pixels = image.load()
    return sum(max(abs(pixels[x,y][i]-accent[i]) for i in range(3)) < 16 for x in range(image.width) for y in range(image.height))


@pytest.mark.p1
@pytest.mark.parametrize("theme,accent", [("dark", (114,174,230)), ("light", (40,103,168))])
@pytest.mark.parametrize("window_name", ["main", "logs"])
def test_pointer_hides_ring_and_keyboard_navigation_restores_it(app_factory, isolated_git, theme, accent, window_name):
    import pyautogui
    root = isolated_git / "workspace"
    make_repo(root / "project")
    app = app_factory(root)
    app.click(f"theme-{theme}")
    target, node_id = (app, "repo-filter-all") if window_name == "main" else (open_logs(app), f"logs-theme-{theme}")
    target.click(node_id)
    wait_for(lambda: top_border_pixels(target, node_id, accent) <= 3, timeout=4, description="mouse click without focus border")
    pyautogui.press("tab")
    pyautogui.hotkey("shift", "tab")
    wait_for(lambda: top_border_pixels(target, node_id, accent) >= 12, timeout=4, description="keyboard focus border visible")
    target.click(node_id)
    wait_for(lambda: top_border_pixels(target, node_id, accent) <= 3, timeout=4, description="mouse hides keyboard border again")
