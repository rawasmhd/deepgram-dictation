//! Deepgram dictation, Rust rewrite.
//!
//!   Alt+M        start dictating
//!   Alt+M        stop, transcribe, paste at the cursor
//!   Ctrl+Alt+Q   quit
//!
//! Not ported yet: live paste and undo (#9), setup and logging (#10).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod config;
mod deepgram;
mod overlay;
mod paste;

use std::io::Write;
use std::{cell::RefCell, ptr, thread};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Threading::{CreateMutexW, OpenMutexW};
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use overlay::{wide, Overlay, State, TIMER_FRAME};

const HOTKEY_TOGGLE: i32 = 1;
const HOTKEY_QUIT: i32 = 2;
/// Posted by the worker thread when the transcript is ready.
const WM_TRANSCRIPT: u32 = WM_APP + 1;
const MIN_SECONDS: f32 = 0.4; // ignore accidental taps

const PYTHON_MUTEX: &str = "DeepgramDictation_v1"; // created by dictate.py
const OWN_MUTEX: &str = "DeepgramDictation_rs_prototype";
const SYNCHRONIZE: u32 = 0x0010_0000;

type Transcript = Result<String, deepgram::Error>;

struct App {
    hwnd: HWND,
    key: String,
    overlay: Overlay,
    recording: Option<audio::Recording>,
    stream: Option<deepgram::Stream>,
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
            log("already running, exiting");
            return;
        }
    }

    let Some(key) = config::api_key() else {
        alert("No API key found.\n\nPut DEEPGRAM_API_KEY=... in a .env file next to dictation.exe.");
        return;
    };

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
    log(&format!(
        "--- started --- hotkey: ALT+M   quit: CTRL+ALT+Q   mode: {}",
        if config::streaming() { "streaming" } else { "batch" }
    ));

    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            hwnd,
            key,
            overlay: Overlay::new(hwnd),
            recording: None,
            stream: None,
        })
    });

    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    log("stopped");
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
        WM_TRANSCRIPT => {
            // the worker thread gave up ownership of this box
            let result = *Box::from_raw(lp as *mut Transcript);
            with_app(|app| app.on_transcript(result));
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
                self.recording = None;
                self.stream = None;
                unsafe { PostQuitMessage(0) };
            }
            HOTKEY_TOGGLE if self.overlay.is_working() => {} // still transcribing
            HOTKEY_TOGGLE => match self.recording.take() {
                None => self.start(),
                Some(recording) => self.stop(recording),
            },
            _ => {}
        }
    }

    fn start(&mut self) {
        let (stream, sink) = if config::streaming() {
            let (stream, sink) = deepgram::Stream::start(self.key.clone());
            (Some(stream), Some(sink))
        } else {
            (None, None)
        };
        match audio::start(sink) {
            Ok(recording) => {
                self.recording = Some(recording);
                self.stream = stream;
                self.overlay.set_state(State::Recording);
            }
            Err(e) => {
                log(&format!("microphone unavailable: {e}"));
                self.overlay.set_state(State::Notice { text: "Microphone unavailable".into(), error: true });
            }
        }
    }

    fn stop(&mut self, recording: audio::Recording) {
        // stopping closes the audio channel, so the stream starts to finish
        let samples = recording.stop();
        let stream = self.stream.take();
        let seconds = samples.len() as f32 / audio::SAMPLE_RATE as f32;
        if seconds < MIN_SECONDS {
            self.overlay.set_state(State::Hidden);
            return;
        }
        self.overlay.set_state(State::Working);

        // wait for Deepgram off the UI thread, so the meter keeps moving
        let key = self.key.clone();
        let hwnd = self.hwnd as isize;
        thread::spawn(move || {
            let result = transcribe(&key, stream, &samples);
            let boxed = Box::into_raw(Box::new(result)) as isize;
            unsafe {
                if PostMessageW(hwnd as HWND, WM_TRANSCRIPT, 0, boxed) == 0 {
                    drop(Box::from_raw(boxed as *mut Transcript));
                }
            }
        });
    }

    fn on_transcript(&mut self, result: Transcript) {
        match result {
            Ok(text) if text.is_empty() => {
                self.overlay.set_state(State::Notice { text: "No speech detected".into(), error: true });
            }
            Ok(text) => {
                log(&format!("-> {text}"));
                self.overlay.set_state(State::Hidden);
                if !paste::paste(self.hwnd, &text) {
                    self.overlay.set_state(State::Notice { text: "Clipboard busy".into(), error: true });
                }
            }
            Err(e) => {
                log(&format!("transcription failed: {e}"));
                self.overlay.set_state(State::Notice { text: e.message(), error: true });
            }
        }
    }

    fn on_timer(&mut self, id: usize) {
        if id == TIMER_FRAME {
            self.overlay.tick(audio::level());
        }
    }
}

/// Streaming text if there is any, else a batch upload of the recording
/// (for example when the socket never connected).
fn transcribe(key: &str, stream: Option<deepgram::Stream>, samples: &[i16]) -> Transcript {
    if let Some(stream) = stream {
        let text = stream.finish();
        if !text.is_empty() {
            return Ok(text);
        }
        log("streaming produced no text; trying batch fallback");
    }
    deepgram::transcribe(key, samples)
}

pub fn log(msg: &str) {
    println!("{msg}");
    if let Some(path) = config::log_file() {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{msg}");
        }
    }
}

fn alert(msg: &str) {
    log(msg);
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            wide(msg).as_ptr(),
            wide("Deepgram Dictation").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
