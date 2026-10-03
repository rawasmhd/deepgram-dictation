//! The tray icon and its menu.

use std::ptr;
use std::sync::atomic::{AtomicIsize, Ordering};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::art::{self, Dot};
use crate::overlay::wide;
use crate::theme;

/// The tray icon sends its mouse events with this message.
pub const WM_TRAY: u32 = WM_APP + 3;
const ICON_ID: u32 = 1;

pub const CMD_TOGGLE: usize = 10;
pub const CMD_SETTINGS: usize = 11;
pub const CMD_AUTOSTART: usize = 12;
pub const CMD_LOG: usize = 13;
pub const CMD_QUIT: usize = 14;

pub struct Tray {
    hwnd: HWND,
    dot: Dot,
    tip: String,
    icon: HICON,
}

impl Tray {
    pub fn add(hwnd: HWND) -> Tray {
        let mut tray = Tray { hwnd, dot: Dot::None, tip: TIP_READY.into(), icon: ptr::null_mut() };
        tray.icon = make_icon(Dot::None);
        tray.notify(NIM_ADD);
        tray
    }

    /// Show a new state. Does nothing if it did not change.
    pub fn set(&mut self, dot: Dot, tip: &str) {
        if dot == self.dot && tip == self.tip {
            return;
        }
        if dot != self.dot {
            self.dot = dot;
            self.replace_icon();
        }
        self.tip = tip.into();
        self.notify(NIM_MODIFY);
    }

    /// The taskbar changed between light and dark.
    pub fn theme_changed(&mut self) {
        self.replace_icon();
        self.notify(NIM_MODIFY);
    }

    /// Explorer restarted, and the taskbar lost the icon.
    pub fn add_again(&mut self) {
        self.notify(NIM_ADD);
    }

    /// A notification from the tray icon.
    pub fn balloon(&self, title: &str, text: &str) {
        let mut data = self.data();
        data.uFlags = NIF_INFO;
        copy(&mut data.szInfoTitle, title);
        copy(&mut data.szInfo, text);
        data.dwInfoFlags = NIIF_USER | NIIF_LARGE_ICON;
        let big = theme::icon(&art::app_icon(48));
        data.hBalloonIcon = big;
        unsafe {
            Shell_NotifyIconW(NIM_MODIFY, &data);
            DestroyIcon(big);
        }
    }

    fn replace_icon(&mut self) {
        let old = self.icon;
        self.icon = make_icon(self.dot);
        unsafe { DestroyIcon(old) };
    }

    fn notify(&self, action: NOTIFY_ICON_MESSAGE) {
        let mut data = self.data();
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = self.icon;
        copy(&mut data.szTip, &self.tip);
        unsafe { Shell_NotifyIconW(action, &data) };
    }

    fn data(&self) -> NOTIFYICONDATAW {
        let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = self.hwnd;
        data.uID = ICON_ID;
        data
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &self.data());
            DestroyIcon(self.icon);
        }
    }
}

pub const TIP_READY: &str = "Deepgram Dictation (Alt+M)";

fn make_icon(dot: Dot) -> HICON {
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16) as usize;
    theme::icon(&art::tray_icon(size, theme::taskbar_dark(), dot))
}

fn copy(dst: &mut [u16], text: &str) {
    let w: Vec<u16> = text.encode_utf16().take(dst.len() - 1).collect();
    dst[..w.len()].copy_from_slice(&w);
    dst[w.len()] = 0;
}

/// The message Explorer sends when it starts, so apps add their icon again.
pub fn taskbar_created_message() -> u32 {
    unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) }
}

/// What the menu shows.
pub struct MenuState<'a> {
    pub status: &'a str,
    pub recording: bool,
    pub busy: bool,
    pub autostart: bool,
}

/// Show the menu at the cursor. Returns the chosen command, if any.
pub fn menu(hwnd: HWND, state: &MenuState) -> Option<usize> {
    unsafe {
        let m = CreatePopupMenu();
        let item = |flags: MENU_ITEM_FLAGS, id: usize, text: &str| {
            AppendMenuW(m, flags, id, wide(text).as_ptr());
        };
        item(MF_STRING | MF_GRAYED, 0, "Deepgram Dictation");
        item(MF_STRING | MF_GRAYED, 0, state.status);
        item(MF_SEPARATOR, 0, "");
        let toggle = if state.recording { "Stop dictation\tAlt+M" } else { "Start dictation\tAlt+M" };
        item(MF_STRING | if state.busy { MF_GRAYED } else { 0 }, CMD_TOGGLE, toggle);
        item(MF_SEPARATOR, 0, "");
        item(MF_STRING, CMD_SETTINGS, "Settings…");
        item(MF_STRING | if state.autostart { MF_CHECKED } else { 0 }, CMD_AUTOSTART, "Start at login");
        item(MF_STRING, CMD_LOG, "Open log file");
        item(MF_SEPARATOR, 0, "");
        item(MF_STRING, CMD_QUIT, "Quit\tCtrl+Alt+Q");

        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        // without this, the menu does not close when you click elsewhere
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            m,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RIGHTALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            ptr::null(),
        );
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(m);
        (cmd > 0).then_some(cmd as usize)
    }
}

// -- the window to dictate into -------------------------------------------------
//
// A click on the tray icon moves the focus to the taskbar. "Start
// dictation" in the menu must go back to the window the user worked in, so
// this remembers the last foreground window that is not the taskbar or ours.

static LAST_WINDOW: AtomicIsize = AtomicIsize::new(0);

pub fn track_foreground() {
    unsafe {
        LAST_WINDOW.store(GetForegroundWindow() as isize, Ordering::Relaxed);
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            ptr::null_mut(),
            Some(on_foreground),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
    }
}

unsafe extern "system" fn on_foreground(_: HWINEVENTHOOK, _: u32, hwnd: HWND, _: i32, _: i32, _: u32, _: u32) {
    if !hwnd.is_null() && !is_shell(hwnd) && !is_ours(hwnd) {
        LAST_WINDOW.store(hwnd as isize, Ordering::Relaxed);
    }
}

unsafe fn is_ours(hwnd: HWND) -> bool {
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    pid == GetCurrentProcessId()
}

unsafe fn is_shell(hwnd: HWND) -> bool {
    let mut buf = [0u16; 64];
    let n = GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    let class = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
    matches!(
        class.as_str(),
        "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "NotifyIconOverflowWindow"
            | "TopLevelWindowForOverflowXamlIsland"
            | "XamlExplorerHostIslandWindow"
            | "#32768" // a menu
    )
}

/// Give the focus back to the window the user worked in.
pub fn restore_focus() {
    let hwnd = LAST_WINDOW.load(Ordering::Relaxed) as HWND;
    unsafe {
        if !hwnd.is_null() && IsWindow(hwnd) != 0 {
            SetForegroundWindow(hwnd);
        }
    }
}
