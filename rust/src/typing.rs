//! Detects when the user types, for undo (Ctrl+Alt+Z).
//!
//! Undo deletes the last dictation with Backspace, which is only correct
//! while the cursor is still at the end of it. Any key that the user
//! presses after a dictation cancels the undo. RegisterHotKey cannot see
//! other keys, so this needs a low-level keyboard hook. The hook only sets
//! a flag; it never reads or stores which keys were pressed.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::{ptr, thread};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::paste::OWN_KEYS;

static TYPED: AtomicBool = AtomicBool::new(false);

/// Install the hook on its own thread. Windows calls a low-level hook on
/// the thread that installed it, and holds back all keyboard input until it
/// returns, so it must not share a thread with slow work such as a paste.
pub fn install() -> bool {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), GetModuleHandleW(ptr::null()), 0);
        let _ = tx.send(!hook.is_null());
        if hook.is_null() {
            return;
        }
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            DispatchMessageW(&msg);
        }
    });
    rx.recv().unwrap_or(false)
}

/// Start watching from now, after a dictation was pasted.
pub fn reset() {
    TYPED.store(false, Ordering::Relaxed);
}

pub fn typed_since_reset() -> bool {
    TYPED.load(Ordering::Relaxed)
}

unsafe extern "system" fn hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && (wp == WM_KEYDOWN as usize || wp == WM_SYSKEYDOWN as usize) {
        let k = &*(lp as *const KBDLLHOOKSTRUCT);
        if k.dwExtraInfo != OWN_KEYS && !is_modifier(k.vkCode) && !is_undo_hotkey(k.vkCode) {
            TYPED.store(true, Ordering::Relaxed);
        }
    }
    CallNextHookEx(ptr::null_mut(), code, wp, lp)
}

fn is_modifier(vk: u32) -> bool {
    [
        VK_SHIFT, VK_LSHIFT, VK_RSHIFT, VK_CONTROL, VK_LCONTROL, VK_RCONTROL, VK_MENU, VK_LMENU,
        VK_RMENU, VK_LWIN, VK_RWIN,
    ]
    .contains(&(vk as VIRTUAL_KEY))
}

fn is_undo_hotkey(vk: u32) -> bool {
    let down = |k: VIRTUAL_KEY| unsafe { GetAsyncKeyState(k as i32) } < 0;
    vk == u32::from(b'Z') && down(VK_CONTROL) && down(VK_MENU)
}
