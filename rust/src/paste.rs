//! Delivery: put text on the clipboard and press Ctrl+V.

use std::{mem, ptr, thread, time::Duration};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::DataExchange::*;
use windows_sys::Win32::System::Memory::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

const CF_UNICODETEXT: u32 = 13;

/// An unassigned virtual key. Pressing it between Alt down and Alt up stops
/// the target app from treating the Alt release as "open the menu bar".
const VK_MASK: VIRTUAL_KEY = 0xE8;

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

pub fn paste(owner: HWND, text: &str) -> bool {
    if !set_clipboard(owner, text) {
        return false;
    }
    release_modifiers();
    thread::sleep(Duration::from_millis(30));
    send(&[
        key(VK_CONTROL, false),
        key(u16::from(b'V'), false),
        key(u16::from(b'V'), true),
        key(VK_CONTROL, true),
    ]);
    true
}

/// Release Alt, Shift and Win if the user still holds them, so the target
/// app sees a plain Ctrl+V. Only keys that are down are released.
fn release_modifiers() {
    let held: Vec<VIRTUAL_KEY> = [VK_LMENU, VK_RMENU, VK_LSHIFT, VK_RSHIFT, VK_LWIN, VK_RWIN]
        .into_iter()
        .filter(|&vk| unsafe { GetAsyncKeyState(vk as i32) } < 0)
        .collect();
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
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(inputs: &[INPUT]) {
    unsafe {
        SendInput(inputs.len() as u32, inputs.as_ptr(), mem::size_of::<INPUT>() as i32);
    }
}
