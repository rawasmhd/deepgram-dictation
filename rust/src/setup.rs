//! The first-run setup window: the API key and autostart.
//!
//! It opens when no key is found, or with `dictation.exe --setup`. It checks
//! the key with Deepgram before the key is saved.

use std::cell::{Cell, RefCell};
use std::ptr;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::deepgram::{self, Error};
use crate::overlay::wide;

const TITLE: &str = "Deepgram Dictation - Setup";
const ID_SAVE: usize = 1; // IDOK, so Enter saves
const ID_CANCEL: usize = 2; // IDCANCEL, so Esc cancels
const BST_UNCHECKED: usize = 0;
const BST_CHECKED: usize = 1;

pub struct Choice {
    pub key: String,
    pub autostart: bool,
}

thread_local! {
    static EDIT: Cell<HWND> = const { Cell::new(ptr::null_mut()) };
    static CHECK: Cell<HWND> = const { Cell::new(ptr::null_mut()) };
    static STATUS: Cell<HWND> = const { Cell::new(ptr::null_mut()) };
    static DONE: Cell<bool> = const { Cell::new(false) };
    static RESULT: RefCell<Option<Choice>> = const { RefCell::new(None) };
}

/// Show the window and wait. None if the user cancelled.
pub fn run(autostart: bool) -> Option<Choice> {
    DONE.set(false);
    RESULT.set(None);
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let class = wide("DictationSetupRs");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: ptr::null_mut(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_BTNFACE + 1) as usize as HBRUSH,
            lpszMenuName: ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);

        let s = GetDpiForSystem() as f32 / 96.0;
        let px = |v: f32| (v * s).round() as i32;
        let style = WS_CAPTION | WS_SYSMENU;
        let mut rect = RECT { left: 0, top: 0, right: px(440.0), bottom: px(196.0) };
        AdjustWindowRectEx(&mut rect, style, 0, 0);
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide(TITLE).as_ptr(),
            style,
            (GetSystemMetrics(SM_CXSCREEN) - w) / 2,
            (GetSystemMetrics(SM_CYSCREEN) - h) / 2,
            w,
            h,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        );
        if hwnd.is_null() {
            return None;
        }

        let font = CreateFontW(
            -px(9.0 * 96.0 / 72.0), 0, 0, 0, FW_NORMAL as _, 0, 0, 0, DEFAULT_CHARSET as _,
            OUT_DEFAULT_PRECIS as _, CLIP_DEFAULT_PRECIS as _, CLEARTYPE_QUALITY as _, 0,
            wide("Segoe UI").as_ptr(),
        );
        let control = |class: &str, text: &str, style: u32, x: f32, y: f32, cw: f32, ch: f32, id: usize| {
            let c = CreateWindowExW(
                0,
                wide(class).as_ptr(),
                wide(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                px(x),
                px(y),
                px(cw),
                px(ch),
                hwnd,
                id as HMENU,
                instance,
                ptr::null(),
            );
            SendMessageW(c, WM_SETFONT, font as WPARAM, 1);
            c
        };
        control("STATIC", "Paste your Deepgram API key.\nGet one at console.deepgram.com, under API Keys.", 0, 16.0, 14.0, 408.0, 36.0, 0);
        let edit = control("EDIT", "", WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32, 16.0, 56.0, 408.0, 26.0, 0);
        let check = control("BUTTON", "Start automatically when I log in", WS_TABSTOP | BS_AUTOCHECKBOX as u32, 16.0, 94.0, 408.0, 24.0, 0);
        let status = control("STATIC", "", 0, 16.0, 124.0, 408.0, 20.0, 0);
        control("BUTTON", "Save", WS_TABSTOP | BS_DEFPUSHBUTTON as u32, 246.0, 154.0, 86.0, 28.0, ID_SAVE);
        control("BUTTON", "Cancel", WS_TABSTOP | BS_PUSHBUTTON as u32, 338.0, 154.0, 86.0, 28.0, ID_CANCEL);
        SendMessageW(check, BM_SETCHECK, if autostart { BST_CHECKED } else { BST_UNCHECKED } as WPARAM, 0);
        EDIT.set(edit);
        CHECK.set(check);
        STATUS.set(status);

        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetFocus(edit);

        let mut msg: MSG = std::mem::zeroed();
        while !DONE.get() && GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            // Tab, Enter and Esc, like in a dialog box
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        DeleteObject(font);
    }
    RESULT.take()
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND if wp & 0xFFFF == ID_SAVE => {
            save(hwnd);
            0
        }
        WM_COMMAND if wp & 0xFFFF == ID_CANCEL => {
            DestroyWindow(hwnd);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            DONE.set(true);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn save(hwnd: HWND) {
    let edit = EDIT.get();
    let len = GetWindowTextLengthW(edit);
    let mut buf = vec![0u16; len as usize + 1];
    GetWindowTextW(edit, buf.as_mut_ptr(), buf.len() as i32);
    let key = String::from_utf16_lossy(&buf[..len as usize]).trim().to_string();
    if key.is_empty() {
        status("Enter a key first.");
        return;
    }

    status("Checking the key with Deepgram...");
    UpdateWindow(hwnd);
    match deepgram::check_key(&key) {
        Ok(()) => {}
        Err(Error::KeyRejected) => {
            status("Deepgram rejected this key. Check it and try again.");
            return;
        }
        Err(e) => {
            status("");
            let answer = MessageBoxW(
                hwnd,
                wide(&format!("Could not check the key ({e}).\n\nSave it anyway?")).as_ptr(),
                wide(TITLE).as_ptr(),
                MB_YESNO | MB_ICONWARNING,
            );
            if answer != IDYES {
                return;
            }
        }
    }
    let autostart = SendMessageW(CHECK.get(), BM_GETCHECK, 0, 0) == BST_CHECKED as LRESULT;
    RESULT.set(Some(Choice { key, autostart }));
    DestroyWindow(hwnd);
}

fn status(text: &str) {
    unsafe { SetWindowTextW(STATUS.get(), wide(text).as_ptr()) };
}
