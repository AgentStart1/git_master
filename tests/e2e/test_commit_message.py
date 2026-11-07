"""Commit body rendering and copying use the decoded immutable message."""
import subprocess
import pytest
from driver import make_repo, git, find, text, assert_copied_text, wait_for


@pytest.mark.p1
@pytest.mark.parametrize("theme", ["dark", "light"])
def test_markdown_body_switches_to_source_and_copies_original(app_factory, isolated_git, theme):
    import pyautogui
    from PIL import ImageChops
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    body = "## Release notes\n\n**Bold** and *italic*, `code`.\n\n- First item\n- Second item\n\n> Read carefully\n\n```text\nhello world\n```\n\n- [x] Checked\n- [ ] Pending\n\n| Name | Status |\n|---|---|\n| Build | Ready |\n\n[Documentation](https://example.invalid)"
    git(repo, "commit", "--allow-empty", "-m", "Markdown message", "-m", body)
    oid = git(repo, "rev-parse", "HEAD").strip()
    index_before = (repo / ".git" / "index").read_bytes()
    app = app_factory(root)
    app.select(0, repo)
    app.click(f"theme-{theme}")
    app.click(f"log-commit-{oid}")
    tree = app.wait(lambda t: (find(t, "commit-message-body") or {}).get("bounds"), "rendered commit body")
    bounds = find(tree, "commit-message-body")["bounds"]
    x, y, width, height = app.window_geometry()
    logical = find(tree, "root")["bounds"]
    region = (round(x + bounds["x"] * width / logical["width"]),
              round(y + bounds["y"] * height / logical["height"]),
              round(bounds["width"] * width / logical["width"]),
              min(90, round(bounds["height"] * height / logical["height"])))
    rendered = pyautogui.screenshot(region=region)
    pyautogui.screenshot().save(app.artifacts / f"markdown-{theme}.png")
    assert_copied_text(app, "commit-message-copy", body)
    app.click("commit-message-source")
    wait_for(lambda: ImageChops.difference(rendered, pyautogui.screenshot(region=region)).getbbox(), description="source differs from formatted body")
    source_pixels = pyautogui.screenshot(region=region)
    pyautogui.screenshot().save(app.artifacts / f"source-{theme}.png")
    assert_copied_text(app, "commit-message-copy", body)
    app.click("commit-message-source")
    wait_for(lambda: ImageChops.difference(source_pixels, pyautogui.screenshot(region=region)).getbbox(), description="formatted body restored")
    pyautogui.moveTo(region[0] + region[2] // 2, region[1] + 20)
    pyautogui.scroll(-15 * 120)
    app.wait(lambda t: (find(t, "commit-message-body") or {}).get("bounds", {}).get("y", bounds["y"]) < bounds["y"], "body scrolled to remaining Markdown")
    pyautogui.screenshot().save(app.artifacts / f"markdown-tail-{theme}.png")
    assert git(repo, "rev-parse", "HEAD").strip() == oid
    assert (repo / ".git" / "index").read_bytes() == index_before


@pytest.mark.p1
def test_legacy_commit_body_is_decoded_before_display_and_copy(app_factory, isolated_git):
    root = isolated_git / "workspace"
    repo = make_repo(root / "project")
    parent = git(repo, "rev-parse", "HEAD").strip()
    tree = git(repo, "rev-parse", "HEAD^{tree}").strip()
    source = f"tree {tree}\nparent {parent}\nauthor André <test@example.invalid> 1700000000 +0000\ncommitter Test <test@example.invalid> 1700000000 +0000\nencoding ISO-8859-1\n\nRésumé\n\n**Café**\n"
    oid = subprocess.check_output(["git", "hash-object", "-t", "commit", "-w", "--stdin"], input=source.encode("latin1"), cwd=repo).decode().strip()
    git(repo, "update-ref", "HEAD", oid)
    git(repo, "config", "i18n.logOutputEncoding", "ISO-8859-1")
    app = app_factory(root)
    app.select(0, repo)
    assert "Résumé" in text(app.node(f"log-commit-{oid}"))
    assert "André" in text(app.node(f"log-commit-{oid}"))
    app.click(f"log-commit-{oid}")
    app.wait(lambda t: (find(t, "commit-message-copy") or {}).get("bounds"), "decoded body")
    assert_copied_text(app, "commit-message-copy", "**Café**")
    import pyautogui
    pyautogui.screenshot().save(app.artifacts / "legacy-encoding.png")
    assert git(repo, "rev-parse", "HEAD").strip() == oid
