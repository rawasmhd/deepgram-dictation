//! Deepgram dictation, Rust rewrite - prototype.
//!
//!   Alt+M        start "recording" (the meter shows the live mic level)
//!   Alt+M        stop; after a short "Transcribing" animation a test
//!                sentence is pasted at the cursor
//!   Ctrl+Alt+Q   quit
//!
//! This prototype checks the hard parts of the rewrite: the global hotkey,
//! the paste, and the floating meter. It does not call Deepgram yet.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod mic;
mod overlay;
mod paste;

use std::{cell::RefCell, ptr, time::Instant};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Threading::{CreateMutexW, OpenMutexW};
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use overlay::{wide, Overlay, State, TIMER_FRAME};

const HOTKEY_TOGGLE: i32 = 1;
const HOTKEY_QUIT: i32 = 2;
const TIMER_WORK: usize = 2;
const FAKE_TRANSCRIBE_MS: u32 = 600;
const MIN_SECONDS: f32 = 0.4; // ignore accidental taps

const PYTHON_MUTEX: &str = "DeepgramDictation_v1"; // created by dictate.py
const OWN_MUTEX: &str = "DeepgramDictation_rs_prototype";
const SYNCHRONIZE: u32 = 0x0010_0000;

struct App {
    hwnd: HWND,
    overlay: Overlay,
    mic: Option<mic::Mic>,
    recording_since: Option<Instant>,
    seconds: f32,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn main() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        let python = OpenMutexW(SYNCHRONIZE, 0, wide(PYTHON_MUTEX).as_ptr());
        if !python.is_null() {
            CloseHandle(python);
            alert("The Python version is running.\n\nStop it first with scripts\\Stop Dictation.bat.");
            return;
        }
        CreateMutexW(ptr::null(), 0, wide(OWN_MUTEX).as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            println!("already running, exiting");
            return;
        }
    }

    let hwnd = overlay::create_window(Some(wndproc));
    if hwnd.is_null() {
        alert("Could not create the meter window.");
        return;
    }

    unsafe {
        let ok = RegisterHotKey(hwnd, HOTKEY_TOGGLE, MOD_ALT | MOD_NOREPEAT, u32::from(b'M')) != 0
            && RegisterHotKey(hwnd, HOTKEY_QUIT, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, u32::from(b'Q'))
                != 0;
        if !ok {
            alert("Alt+M or Ctrl+Alt+Q is already used by another app.");
            return;
        }
    }
    println!("hotkey: ALT+M   quit: CTRL+ALT+Q");

    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            hwnd,
            overlay: Overlay::new(hwnd),
            mic: None,
            recording_since: None,
            seconds: 0.0,
        })
    });

    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    println!("stopped");
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            with_app(|app| app.on_hotkey(wp as i32));
            0
        }
        WM_TIMER => {
            with_app(|app| app.on_timer(wp));
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// Window calls can re-enter the window procedure, so skip if busy.
fn with_app(f: impl FnOnce(&mut App)) {
    APP.with(|a| {
        if let Ok(mut guard) = a.try_borrow_mut() {
            if let Some(app) = guard.as_mut() {
                f(app);
            }
        }
    });
}

impl App {
    fn on_hotkey(&mut self, id: i32) {
        match id {
            HOTKEY_QUIT => {
                self.mic = None;
                unsafe { PostQuitMessage(0) };
            }
            HOTKEY_TOGGLE if self.overlay.is_working() => {} // still busy
            HOTKEY_TOGGLE => match self.recording_since.take() {
                None => self.start(),
                Some(since) => self.stop(since.elapsed().as_secs_f32()),
            },
            _ => {}
        }
    }

    fn start(&mut self) {
        match mic::start() {
            Ok(m) => {
                self.mic = Some(m);
                self.recording_since = Some(Instant::now());
                self.overlay.set_state(State::Recording);
            }
            Err(e) => {
                println!("microphone unavailable: {e}");
                self.overlay.set_state(State::Notice { text: "Microphone unavailable".into(), error: true });
            }
        }
    }

    fn stop(&mut self, seconds: f32) {
        self.mic = None;
        if seconds < MIN_SECONDS {
            self.overlay.set_state(State::Hidden);
            return;
        }
        self.seconds = seconds;
        self.overlay.set_state(State::Working);
        unsafe { SetTimer(self.hwnd, TIMER_WORK, FAKE_TRANSCRIBE_MS, None) };
    }

    fn on_timer(&mut self, id: usize) {
        match id {
            TIMER_FRAME => self.overlay.tick(mic::level()),
            TIMER_WORK => {
                unsafe { KillTimer(self.hwnd, TIMER_WORK) };
                self.overlay.set_state(State::Hidden);
                let text = format!("Rust prototype test: {:.1} seconds recorded.", self.seconds);
                println!("-> {text}");
                if !paste::paste(self.hwnd, &text) {
                    self.overlay.set_state(State::Notice { text: "Clipboard busy".into(), error: true });
                }
            }
            _ => {}
        }
    }
}

fn alert(msg: &str) {
    println!("{msg}");
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            wide(msg).as_ptr(),
            wide("Deepgram Dictation").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
