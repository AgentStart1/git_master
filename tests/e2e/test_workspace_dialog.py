"""Native Windows folder selection; Linux portal dialogs need a desktop session."""
import ctypes
from ctypes import wintypes
import sys
import json
import subprocess
import pytest
from driver import find, make_repo, text, wait_for


def folder_dialog(app):
    user32 = ctypes.windll.user32
    user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user32.IsWindowVisible.argtypes = [wintypes.HWND]
    user32.GetClassNameW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    dialogs = []
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def visit(handle, _):
        pid, name = wintypes.DWORD(), ctypes.create_unicode_buffer(256)
        user32.GetWindowThreadProcessId(handle, ctypes.byref(pid))
        user32.GetClassNameW(handle, name, 256)
        if pid.value == app.proc.pid and name.value == "#32770" and user32.IsWindowVisible(handle):
            dialogs.append(handle)
        return True
    user32.EnumWindows(callback_type(visit), 0)
    return dialogs[0] if len(dialogs) == 1 else None


def folder_shows_item(handle, value):
    # Read only this test dialog. Do not treat queued keyboard input as delivered
    # or an address submission as completed shell navigation.
    script = r'''
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$request = [Console]::In.ReadToEnd() | ConvertFrom-Json
try {
$dialog = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$request.handle)
$items = $dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants,
    [System.Windows.Automation.Condition]::TrueCondition)
foreach ($item in $items) {
    $matched = $item.Current.Name -eq $request.value
    if ($matched) {
        [Console]::Write('ready')
        break
    }
}
} catch {
    # Shell navigation can replace controls during this read. Retry the observation,
    # but keep all other automation failures visible and still require the item.
    if ($_.Exception -is [System.Windows.Automation.ElementNotAvailableException] -or
        $_.Exception.InnerException -is [System.Windows.Automation.ElementNotAvailableException]) {
        exit 0
    }
    throw
}
'''
    result = subprocess.run(["powershell", "-STA", "-NoProfile", "-NonInteractive", "-Command", script],
        input=json.dumps({"handle": handle, "value": str(value)}), text=True, capture_output=True, timeout=5,
        creationflags=subprocess.CREATE_NO_WINDOW)
    assert result.returncode == 0, result.stderr
    return result.stdout.strip() == "ready"


def active_folder_dialog(app):
    handle = folder_dialog(app)
    return handle if handle and ctypes.windll.user32.GetForegroundWindow() == handle else None


def fill_address(handle, path):
    # Fill the focused native Edit control using its normal text-input message.
    # This avoids keyboard-layout/IME ambiguity without bypassing folder selection.
    class GuiThread(ctypes.Structure):
        _fields_ = [("size", wintypes.DWORD), ("flags", wintypes.DWORD),
            *[(name, wintypes.HWND) for name in ("active", "focus", "capture", "menu", "move", "caret")],
            ("rect", wintypes.RECT)]
    user32 = ctypes.windll.user32
    user32.GetGUIThreadInfo.argtypes = [wintypes.DWORD, ctypes.POINTER(GuiThread)]
    user32.IsChild.argtypes = [wintypes.HWND, wintypes.HWND]
    info = GuiThread()
    info.size = ctypes.sizeof(info)
    thread = user32.GetWindowThreadProcessId(handle, None)
    if not user32.GetGUIThreadInfo(thread, ctypes.byref(info)) or not user32.IsChild(handle, info.focus):
        return False
    kind = ctypes.create_unicode_buffer(256)
    user32.GetClassNameW(info.focus, kind, 256)
    if kind.value != "Edit":
        return False
    user32.SendMessageTimeoutW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM,
        wintypes.LPARAM, wintypes.UINT, wintypes.UINT, ctypes.POINTER(ctypes.c_size_t)]
    value, result = ctypes.create_unicode_buffer(str(path)), ctypes.c_size_t()
    assert user32.SendMessageTimeoutW(info.focus, 0x000C, 0, ctypes.addressof(value), 2, 1000, ctypes.byref(result))
    actual = ctypes.create_unicode_buffer(4096)
    assert user32.SendMessageTimeoutW(info.focus, 0x000D, 4096, ctypes.addressof(actual), 2, 1000, ctypes.byref(result))
    return actual.value == str(path)


@pytest.mark.p1
@pytest.mark.skipif(sys.platform != "win32", reason="Native folder dialog automation currently Windows-only")
def test_native_workspace_picker_cancel_and_switch_clear_previous_selection(app_factory, isolated_git):
    import pyautogui
    first_root, second_root = isolated_git / "first", isolated_git / "second"
    first = make_repo(first_root / "old-project")
    second = make_repo(second_root / "new-project")
    app = app_factory(first_root)
    app.select(0, first)
    app.tab("change")
    app.click("change-dir-btn")
    wait_for(lambda: active_folder_dialog(app), description="active native workspace picker")
    pyautogui.press("esc")
    wait_for(lambda: not folder_dialog(app), description="cancelled native picker")
    assert app.tree()["state"]["path"] == str(first)
    app.click("change-dir-btn")
    handle = wait_for(lambda: active_folder_dialog(app), description="active native workspace picker")
    pyautogui.hotkey("alt", "d")
    wait_for(lambda: fill_address(handle, second_root), description="native address field filled")
    pyautogui.press("enter")
    wait_for(lambda: folder_shows_item(handle, second.name), description="requested folder contents loaded")
    user32 = ctypes.windll.user32
    user32.GetDlgItem.argtypes = [wintypes.HWND, ctypes.c_int]
    user32.GetDlgItem.restype = wintypes.HWND
    user32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    def ok_button():
        button = user32.GetDlgItem(handle, 1)
        rect = wintypes.RECT()
        if button and user32.GetWindowRect(button, ctypes.byref(rect)):
            return rect
    rect = wait_for(ok_button, description="Select Folder button")
    pyautogui.click((rect.left + rect.right) // 2, (rect.top + rect.bottom) // 2)
    wait_for(lambda: not folder_dialog(app), description="accepted native picker")
    app.wait(lambda t: not t["state"]["scanning"] and "new-project" in text(t), "new workspace scan")
    assert app.tree()["state"]["path"] is None
    assert "old-project" not in text(find(app.tree(), "repo-list"))
    app.select(0, second)
    app.tab("change")
    assert "Working tree clean" in text(app.tree())


@pytest.mark.p1
@pytest.mark.skipif(sys.platform != "win32", reason="Native folder dialog automation currently Windows-only")
def test_cancelling_workspace_picker_does_not_cancel_inflight_scan(app_factory, isolated_git, monkeypatch):
    import pyautogui
    root = isolated_git / "workspace"
    make_repo(root / "project")
    gate = isolated_git / "scan-gate"
    gate.mkdir()
    monkeypatch.setenv("GIT_MASTER_TEST_SCAN_GATE", str(gate))
    app = app_factory(root, wait_scan=False)
    try:
        wait_for(lambda: (gate / "entered").exists(), description="scan result held before publication")
        assert app.tree()["state"]["scanning"]
        app.click("change-dir-btn")
        wait_for(lambda: active_folder_dialog(app), description="active workspace picker during scan")
        pyautogui.press("esc")
        wait_for(lambda: not folder_dialog(app), description="cancelled picker")
        (gate / "release").touch()
        app.wait(lambda t: not t["state"]["scanning"] and "project" in text(find(t, "repo-list") or {}), "original scan survives picker cancellation")
    finally:
        (gate / "release").touch()
