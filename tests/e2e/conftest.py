from pathlib import Path
import os
import subprocess
import sys
import pytest
from driver import App

ROOT = Path(__file__).resolve().parents[2]


def pytest_addoption(parser):
    parser.addoption("--app-binary", help="Prebuilt test-rpc executable; otherwise build once")
    parser.addoption("--artifacts", default="test-results/e2e", help="Failure logs, view trees and window screenshots")


@pytest.fixture(scope="session")
def binary(request):
    if sys.platform not in ("win32", "linux"):
        pytest.skip("Desktop driver supports Windows and Linux/X11")
    value = request.config.getoption("--app-binary")
    if value:
        path = Path(value).resolve()
        assert path.is_file(), path
        return path
    # Dedicated output avoids overwriting a developer's running application on Windows.
    path = ROOT / "target" / "debug" / ("git_master_e2e.exe" if sys.platform == "win32" else "git_master_e2e")
    subprocess.run(["cargo", "rustc", "--features", "test-rpc", "--bin", "git_master", "--", "-o", str(path)], cwd=ROOT, check=True)
    assert path.is_file(), path
    return path


@pytest.fixture
def isolated_git(tmp_path, monkeypatch):
    # Local-only remotes and synthetic identities; do not inherit personal Git configuration.
    config = tmp_path / "empty-gitconfig"
    config.touch()
    for key in list(os.environ):
        if key.startswith("GIT_"):
            monkeypatch.delenv(key)
    monkeypatch.setenv("GIT_CONFIG_GLOBAL", str(config))
    monkeypatch.setenv("GIT_CONFIG_NOSYSTEM", "1")
    monkeypatch.setenv("GIT_TERMINAL_PROMPT", "0")
    monkeypatch.setenv("GIT_ALLOW_PROTOCOL", "file")
    return tmp_path


@pytest.fixture
def app_factory(binary, request, isolated_git):
    apps = []
    def launch(workspace, **options):
        folder = Path(request.config.getoption("--artifacts")) / request.node.name
        app = App(binary, workspace, folder, os.environ.copy(), **options)
        apps.append(app)
        return app
    yield launch
    for app in apps:
        if getattr(request.node, "failed", False):
            app.save_failure()
        app.close()


@pytest.hookimpl(hookwrapper=True)
def pytest_runtest_makereport(item, call):
    outcome = yield
    report = outcome.get_result()
    if report.failed:
        item.failed = True
