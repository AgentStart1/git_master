"""Real desktop input plus read-only view-tree observations. No direct feature actions."""
from __future__ import annotations
import ctypes
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import time


def walk(node):
    yield node
    for child in node.get("children", []):
        yield from walk(child)


def find(tree, node_id):
    return next((node for node in walk(tree) if node.get("id") == node_id), None)


def text(tree):
    return "\n".join(node.get("text", "") for node in walk(tree))


def status(tree):
    return text(find(tree, "status-message") or {})


def wait_for(predicate, *, timeout=15, description="condition"):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            value = predicate()
            if value:
                return value
        except (OSError, ValueError, KeyError) as error:
            last = error
        time.sleep(0.05)
    raise AssertionError(f"Timed out waiting for {description}; last error: {last}")


class App:
    def __init__(self, binary, workspace, artifacts, env, *, wait_scan=True):
        self.artifacts = artifacts
        artifacts.mkdir(parents=True, exist_ok=True)
        self.output = (artifacts / "app.log").open("wb")
        with socket.socket() as reserve:
            reserve.bind(("127.0.0.1", 0))
            self.port = reserve.getsockname()[1]
        config_dir = (artifacts / "git-config").resolve()
        config_dir.mkdir(exist_ok=True)
        self.proc = subprocess.Popen([str(binary)], env={**env, "GIT_MASTER_OPEN_DIR": str(workspace),
            "GIT_MASTER_RPC_PORT": str(self.port), "GIT_MASTER_TEST_PROMPTS": "1",
            "GIT_MASTER_TEST_CONFIG_DIR": str(config_dir),
            "LOCALAPPDATA": str((artifacts / "state").resolve())},
            stdout=self.output, stderr=subprocess.STDOUT)
        self.last_tree = None
        self.window = None
        self.rpc_id = 0
        try:
            wait_for(lambda: self.tree(), description="application RPC/frame")
            # Xvfb has no window manager to focus new windows. GPUI can keep an
            # unfocused window idle, so focus before waiting for painted controls.
            self.window_geometry()
            if wait_scan:
                self.wait(lambda tree: str(workspace) in text(tree) and not tree["state"]["scanning"], "workspace scan")
        except BaseException:
            self.save_failure()
            self.close()
            raise

    def rpc(self, method, params=None):
        self.rpc_id += 1
        with socket.create_connection(("127.0.0.1", self.port), timeout=2) as connection:
            connection.settimeout(4)
            with connection.makefile("rwb") as channel:
                channel.write((json.dumps({"jsonrpc": "2.0", "id": self.rpc_id,
                    "method": method, "params": params or {}}) + "\n").encode())
                channel.flush()
                response = json.loads(channel.readline())
        assert response.get("id") == self.rpc_id, response
        assert "error" not in response, response
        return response.get("result")

    def tree(self):
        assert self.proc.poll() is None, f"Application exited {self.proc.returncode}; see {self.artifacts / 'app.log'}"
        tree = self.rpc("get_view_tree")
        if tree:
            self.last_tree = tree
        return tree

    def wait(self, predicate, description="application state"):
        return wait_for(lambda: (tree if predicate(tree) else None) if (tree := self.tree()) else None,
            description=description)

    @staticmethod
    def idle(tree):
        state = tree.get("state", {})
        return state and not any(state.get(key) for key in ("busy", "loading_detail", "worktree_loading", "scanning"))

    def node(self, node_id):
        return find(self.tree(), node_id)

    def window_geometry(self):
        if sys.platform == "win32":
            from ctypes import wintypes
            user32 = ctypes.windll.user32
            # Pointer-sized types are required on 64-bit Windows.
            user32.GetForegroundWindow.restype = wintypes.HWND
            user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
            user32.IsWindowVisible.argtypes = [wintypes.HWND]
            user32.SetForegroundWindow.argtypes = [wintypes.HWND]
            user32.SetWindowPos.argtypes = [wintypes.HWND, wintypes.HWND, ctypes.c_int, ctypes.c_int,
                ctypes.c_int, ctypes.c_int, wintypes.UINT]
            user32.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
            user32.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
            candidates = []
            callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
            def candidate(handle, _):
                pid = wintypes.DWORD()
                user32.GetWindowThreadProcessId(handle, ctypes.byref(pid))
                if pid.value == self.proc.pid and user32.IsWindowVisible(handle):
                    title = getattr(self, "window_title", None)
                    if title is not None:
                        label = ctypes.create_unicode_buffer(256)
                        user32.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
                        user32.GetWindowTextW(handle, label, 256)
                        if label.value != title:
                            return True
                    candidates.append(handle)
                return True
            user32.EnumWindows(callback_type(candidate), 0)
            assert len(candidates) == 1, f"Expected one test window, found {candidates}"
            self.window = candidates[0]
            user32.SetForegroundWindow(self.window)
            def focused():
                pid = wintypes.DWORD()
                user32.GetWindowThreadProcessId(user32.GetForegroundWindow(), ctypes.byref(pid))
                return pid.value == self.proc.pid
            if not focused():
                # Windows can reject programmatic activation from a background runner.
                # Raise only our window, then activate it with a real click on its header.
                import pyautogui
                assert user32.SetWindowPos(self.window, -1, 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0010)
                try:
                    header = wintypes.POINT(20, 20)
                    assert user32.ClientToScreen(self.window, ctypes.byref(header))
                    user32.WindowFromPoint.argtypes = [wintypes.POINT]
                    user32.WindowFromPoint.restype = wintypes.HWND
                    target = user32.WindowFromPoint(header)
                    target_pid = wintypes.DWORD()
                    user32.GetWindowThreadProcessId(target, ctypes.byref(target_pid))
                    assert target_pid.value == self.proc.pid, "activation point must belong to test process"
                    pyautogui.click(header.x, header.y)
                finally:
                    user32.SetWindowPos(self.window, -2, 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0010)
            wait_for(focused, timeout=3, description="test window foreground")
            rect, origin = wintypes.RECT(), wintypes.POINT(0, 0)
            assert user32.GetClientRect(self.window, ctypes.byref(rect))
            assert user32.ClientToScreen(self.window, ctypes.byref(origin))
            return origin.x, origin.y, rect.right, rect.bottom
        if sys.platform.startswith("linux"):
            def xdo(*args):
                return subprocess.check_output(["xdotool", *map(str, args)], text=True, timeout=3).strip()
            title = getattr(self, "window_title", None)
            if title is not None:
                import re
                windows = xdo("search", "--onlyvisible", "--all", "--pid", self.proc.pid, "--name", "^" + re.escape(title) + "$").splitlines()
            else:
                windows = xdo("search", "--onlyvisible", "--pid", self.proc.pid).splitlines()
            assert len(windows) == 1, windows
            self.window = windows[0]
            # Xvfb has no window manager: focusing alone leaves another native
            # window above this one, so clicks can land in the Logs window.
            xdo("windowraise", self.window)
            xdo("windowfocus", self.window)
            values = dict(line.split("=", 1) for line in xdo("getwindowgeometry", "--shell", self.window).splitlines() if "=" in line)
            return tuple(int(values[key]) for key in ("X", "Y", "WIDTH", "HEIGHT"))
        raise AssertionError("Desktop input currently supports Windows and Linux/X11")

    def point(self, node_id, fraction=(0.5, 0.5)):
        # Importing on Windows sets process DPI awareness before translating coordinates.
        import pyautogui
        pyautogui.PAUSE = 0.05
        tree = self.wait(lambda tree: (node := find(tree, node_id)) and node.get("interactive") and node.get("bounds"), f"clickable {node_id}")
        bounds = find(tree, node_id)["bounds"]
        x, y, width, height = self.window_geometry()
        root = (find(tree, "root") or find(tree, "logs-root"))["bounds"]
        scale_x, scale_y = width / root["width"], height / root["height"]
        local_x, local_y = bounds["x"] + bounds["width"] * fraction[0], bounds["y"] + bounds["height"] * fraction[1]
        assert 0 < local_x < root["width"] and 0 < local_y < root["height"], f"{node_id} outside viewport"
        return round(x + local_x * scale_x), round(y + local_y * scale_y)

    def click(self, node_id, *, button="left", await_operation=True):
        import pyautogui
        position = self.point(node_id)
        before = self.tree()
        pyautogui.click(*position, button=button)
        if await_operation and button == "left" and self.is_operation(node_id, before):
            self.wait_completion(before["state"]["status_revision"])

    @staticmethod
    def is_operation(node_id, tree):
        if node_id in ("fetch-all-btn", "switch-all-main-btn"):
            return find(tree, "repo-0") is not None
        return node_id == "worktree-refresh" or node_id == "stash-save" or node_id.startswith((
            "change-stage-", "change-unstage-", "change-diff-", "stash-show-", "stash-select-",
            "stash-apply-", "stash-pop-", "log-checkout-")) or (
            node_id.startswith("log-remote-") and node_id.endswith(("-fetch", "-pull", "-push")))

    def wait_completion(self, revision):
        return self.wait(lambda tree: self.idle(tree) and tree["state"]["status_revision"] > revision,
            "new operation completion (not a previous status)")

    def drag(self, node_id, dx, dy, *, fraction=(0.5, 0.5)):
        import pyautogui
        pyautogui.moveTo(*self.point(node_id, fraction))
        pyautogui.dragRel(dx, dy, duration=0.4)

    def scroll(self, node_id, steps):
        import pyautogui
        pyautogui.moveTo(*self.point(node_id, (0.9, 0.7)))
        # PyAutoGUI 0.9.54 forwards raw wheel units on Windows; one notch is 120.
        pyautogui.scroll(steps * 120 if sys.platform == "win32" else steps)

    def select(self, index, path):
        self.click(f"repo-{index}")
        return self.wait(lambda tree: self.idle(tree) and tree["state"].get("path") and
            Path(tree["state"]["path"]).resolve() == Path(path).resolve(), f"repository {path}")

    def tab(self, name):
        self.click(f"tab-{name}")
        return self.wait(lambda tree: self.idle(tree) and tree["state"]["tab"] == name, f"tab {name}")

    def change(self, repo, relative, action):
        assert Path(self.tree()["state"]["path"]).resolve() == Path(repo).resolve(), "wrong repository selected"
        tree = self.wait(lambda tree: self.idle(tree) and any(node.get("state", {}).get("path") == relative for node in walk(tree)), f"file row {relative}")
        entry = next(node["state"] for node in walk(tree) if node.get("state", {}).get("path") == relative)
        self.click(f"change-{action}-{entry['index']}")

    def prompt(self, accept, *, await_operation=True):
        tree = self.wait(lambda tree: find(tree, "test-prompt"), "confirmation dialog")
        revision = tree["state"]["status_revision"]
        self.click(f"test-prompt-answer-{0 if accept else 1}")
        if not await_operation:
            return self.wait(lambda tree: not find(tree, "test-prompt"), "answered confirmation")
        self.wait(lambda tree: self.idle(tree) and not find(tree, "test-prompt"), "confirmation completion")
        return self.wait_completion(revision)

    def save_failure(self):
        try:
            tree = self.tree()
        except BaseException:
            tree = self.last_tree
        (self.artifacts / "view-tree.json").write_text(json.dumps(tree, ensure_ascii=False, indent=2), encoding="utf-8")
        try:
            import pyautogui
            x, y, width, height = self.window_geometry()
            pyautogui.screenshot(region=(x, y, width, height)).save(self.artifacts / "window.png")
        except Exception as error:
            (self.artifacts / "screenshot-error.txt").write_text(str(error), encoding="utf-8")

        try:
            logs = self.rpc("get_logs_view_tree")
            if logs:
                (self.artifacts / "logs-view-tree.json").write_text(json.dumps(logs, ensure_ascii=False, indent=2), encoding="utf-8")
                import copy
                log_window = copy.copy(self)
                log_window.window_title = "Git Master Logs"
                x, y, width, height = log_window.window_geometry()
                pyautogui.screenshot(region=(x, y, width, height)).save(self.artifacts / "logs-window.png")
        except Exception as error:
            (self.artifacts / "logs-diagnostics-error.txt").write_text(str(error), encoding="utf-8")

    def close(self):
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=5)
        self.output.close()


def git(repo, *args, check=True):
    result = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True,
        encoding="utf-8", errors="replace", timeout=20)
    if check:
        assert result.returncode == 0, f"git {args}: {result.stderr}"
    return result.stdout if check else result


def make_repo(path, *, commit=True):
    path.mkdir(parents=True, exist_ok=True)
    git(path, "init", "-b", "main")
    git(path, "config", "user.name", "E2E Tests")
    git(path, "config", "user.email", "e2e@example.invalid")
    git(path, "config", "core.autocrlf", "false")
    if commit:
        (path / "tracked.txt").write_text("base\n", encoding="utf-8")
        commit_all(path, "Base commit")
    return path


def commit_all(path, message):
    git(path, "add", "--all")
    git(path, "commit", "--allow-empty", "-m", message)


def assert_copied_text(app, button_id, expected):
    # Keep Tk's delayed clipboard rendering out of the desktop driver process.
    subprocess.run([sys.executable, "-c",
        "import tkinter; r=tkinter.Tk(); r.withdraw(); r.clipboard_clear(); "
        "r.clipboard_append('unchanged clipboard sentinel'); r.update(); r.destroy()"],
        check=True, timeout=5)
    app.click(button_id)
    def copied_output():
        result = subprocess.run([sys.executable, "-c",
            "import tkinter,json; r=tkinter.Tk(); r.withdraw(); "
            "print(json.dumps(r.clipboard_get())); r.destroy()"],
            capture_output=True, text=True, timeout=5)
        return result.returncode == 0 and json.loads(result.stdout) == expected
    wait_for(copied_output, description="complete clipboard output")
