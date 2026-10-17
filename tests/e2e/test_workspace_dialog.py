"""Native Windows folder selection; Linux portal dialogs need a desktop session."""
import ctypes
from ctypes import wintypes
import sys
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
    wait_for(lambda: folder_dialog(app), description="native workspace picker")
    pyautogui.press("esc")
    wait_for(lambda: not folder_dialog(app), description="cancelled native picker")
    assert app.tree()["state"]["path"] == str(first)
    app.click("change-dir-btn")
    handle = wait_for(lambda: folder_dialog(app), description="native workspace picker")
    pyautogui.hotkey("alt", "d")
    pyautogui.write(str(second_root), interval=0.01)
    pyautogui.press("enter")
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
