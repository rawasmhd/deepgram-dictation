//! Delivery: put text on the clipboard and press Ctrl+V, and undo with
//! Backspace.

use std::{mem, ptr, thread, time::Duration, time::Instant};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::DataExchange::*;
use windows_sys::Win32::System::Memory::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

const CF_UNICODETEXT: u32 = 13;

/// An unassigned virtual key. Pressing it between Alt down and Alt up stops
/// the target app from treating the Alt release as "open the menu bar".
const VK_MASK: VIRTUAL_KEY = 0xE8;

/// Marks the keys that this app sends (in `dwExtraInfo`), so the keyboard
/// hook does not count them as the user typing.
pub const OWN_KEYS: usize = 0x4443_5450; // "DCTP"

/// Programs such as Windows clipboard history open the clipboard right after
/// each change. A Ctrl+V that arrives then pastes nothing (#18), so wait
/// this long at most for the clipboard to be free.
const CLIPBOARD_FREE_TIMEOUT: Duration = Duration::from_millis(250);

pub fn set_clipboard(owner: HWND, text: &str) -> bool {
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        // another app can hold the clipboard for a moment, so retry
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(owner) != 0 {
                opened = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        if !opened {
            return false;
        }

        EmptyClipboard();
        let mut ok = false;
        let mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
        if !mem.is_null() {
            let dst = GlobalLock(mem) as *mut u16;
            if !dst.is_null() {
                ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());
                GlobalUnlock(mem);
                // on success the clipboard owns the memory
                ok = !SetClipboardData(CF_UNICODETEXT, mem).is_null();
            }
            if !ok {
                GlobalFree(mem);
            }
        }
        CloseClipboard();
        ok
    }
}

/// Wait until no program has the clipboard open, so the target app can
/// read it. Opening and closing it without a change notifies nobody.
fn wait_for_free_clipboard(owner: HWND) {
    let end = Instant::now() + CLIPBOARD_FREE_TIMEOUT;
    while Instant::now() < end {
        unsafe {
            if OpenClipboard(owner) != 0 {
                CloseClipboard();
                return;
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
}

pub fn paste(owner: HWND, text: &str) -> bool {
    if !set_clipboard(owner, text) {
        return false;
    }
    release_modifiers(false);
    // let clipboard listeners see the change first, then wait for them
    thread::sleep(Duration::from_millis(30));
    wait_for_free_clipboard(owner);
    send(&[
        key(VK_CONTROL, false),
        key(u16::from(b'V'), false),
        key(u16::from(b'V'), true),
        key(VK_CONTROL, true),
    ]);
    true
}

/// Delete `count` characters before the cursor.
pub fn backspaces(count: usize) {
    // the user may still hold Ctrl+Alt from the undo hotkey, and
    // Ctrl+Backspace would delete whole words
    release_modifiers(true);
    thread::sleep(Duration::from_millis(30));
    let inputs: Vec<INPUT> =
        (0..count).flat_map(|_| [key(VK_BACK, false), key(VK_BACK, true)]).collect();
    send(&inputs);
}

/// Release Alt, Shift and Win (and Ctrl, if asked) if the user still holds
/// them, so the target app sees plain keys. Only keys that are down are
/// released.
fn release_modifiers(include_ctrl: bool) {
    let mut keys = vec![VK_LMENU, VK_RMENU, VK_LSHIFT, VK_RSHIFT, VK_LWIN, VK_RWIN];
    if include_ctrl {
        keys.extend([VK_LCONTROL, VK_RCONTROL]);
    }
    let held: Vec<VIRTUAL_KEY> =
        keys.into_iter().filter(|&vk| unsafe { GetAsyncKeyState(vk as i32) } < 0).collect();
    if held.is_empty() {
        return;
    }
    let mut inputs = vec![key(VK_MASK, false), key(VK_MASK, true)];
    inputs.extend(held.into_iter().map(|vk| key(vk, true)));
    send(&inputs);
}

fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: OWN_KEYS,
            },
        },
    }
}

fn send(inputs: &[INPUT]) {
    unsafe {
        SendInput(inputs.len() as u32, inputs.as_ptr(), mem::size_of::<INPUT>() as i32);
    }
}
