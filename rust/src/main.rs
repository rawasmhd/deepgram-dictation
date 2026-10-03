//! Deepgram dictation, Rust rewrite.
//!
//!   Alt+M        start dictating
//!   Alt+M        stop, transcribe, paste at the cursor
//!   Ctrl+Alt+Z   delete the last dictation, if you have not typed since
//!   Ctrl+Alt+Q   quit
//!
//! The first start, or `dictation.exe --setup`, asks for the API key.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod config;
mod deepgram;
mod live;
mod logging;
mod overlay;
mod paste;
mod setup;
mod typing;

use std::{cell::RefCell, ptr, thread};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use live::LivePaste;
use overlay::{wide, Overlay, State, TIMER_FRAME};

const HOTKEY_TOGGLE: i32 = 1;
const HOTKEY_QUIT: i32 = 2;
const HOTKEY_UNDO: i32 = 3;
/// Posted by the worker thread when the transcript is ready.
const WM_TRANSCRIPT: u32 = WM_APP + 1;
/// Posted by the network thread when a new final phrase arrived.
const WM_LIVE_TEXT: u32 = WM_APP + 2;
/// Puts the full text on the clipboard after the last live paste.
const TIMER_CLIPBOARD: usize = 3;
/// Let the target app read the clipboard for the last Ctrl+V first.
const CLIPBOARD_DELAY_MS: u32 = 500;
/// Also check live paste this often (frames), to see if the focus came back.
const LIVE_CHECK_FRAMES: u32 = 8;
const MIN_SECONDS: f32 = 0.4; // ignore accidental taps

/// The same name as in dictate.py, so only one version runs at a time.
const INSTANCE_MUTEX: &str = "DeepgramDictation_v1";

type Transcript = Result<String, deepgram::Error>;

/// The last dictation, for undo: how many characters, and where.
struct Delivery {
    chars: usize,
    window: HWND,
}

struct App {
    hwnd: HWND,
    key: String,
    overlay: Overlay,
    recording: Option<audio::Recording>,
    stream: Option<deepgram::Stream>,
    transcript: Option<deepgram::Transcript>,
    live: Option<LivePaste>,
    last: Option<Delivery>,
    clipboard_later: Option<String>,
    frames: u32,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn main() {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    logging::init();

    // setup: on the first start (no key), or when asked with --setup
    let asked = std::env::args().any(|a| a == "--setup");
    let mut key = config::api_key();
    let mut did_setup = false;
    if asked || key.is_none() {
        match setup::run(key.is_none() || config::autostart_enabled()) {
            Some(choice) => {
                if let Err(e) = apply_setup(&choice) {
                    alert(&format!("The setup could not be saved.\n\n{e}"));
                    return;
                }
                key = Some(choice.key);
                did_setup = true;
            }
            None if key.is_none() => return, // cancelled, and no key to run with
            None => {}
        }
    }
    let Some(key) = key else { return };

    // one copy at a time, Python or Rust: both use this mutex name
    unsafe {
        CreateMutexW(ptr::null(), 0, wide(INSTANCE_MUTEX).as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            log("already running, exiting");
            if did_setup {
                info("Saved.\n\nDictation is already running. To use the new key, quit it with Ctrl+Alt+Q and start it again.");
            }
            return;
        }
    }

    if !audio::has_input_device() {
        alert("No microphone was found.\n\nConnect one, then start Deepgram Dictation again.");
        return;
    }

    let hwnd = overlay::create_window(Some(wndproc));
    if hwnd.is_null() {
        alert("Could not create the meter window.");
        return;
    }

    unsafe {
        let ok = RegisterHotKey(hwnd, HOTKEY_TOGGLE, MOD_ALT | MOD_NOREPEAT, u32::from(b'M')) != 0
            && RegisterHotKey(hwnd, HOTKEY_QUIT, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, u32::from(b'Q'))
                != 0
            && RegisterHotKey(hwnd, HOTKEY_UNDO, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, u32::from(b'Z'))
                != 0;
        if !ok {
            alert("Alt+M, Ctrl+Alt+Z or Ctrl+Alt+Q is already used by another app.");
            return;
        }
    }
    if !typing::install() {
        log("keyboard hook unavailable; undo works even after typing");
    }
    log(&format!(
        "hotkey: ALT+M   undo: CTRL+ALT+Z   quit: CTRL+ALT+Q   mode: {}   live paste: {}",
        if config::streaming() { "streaming" } else { "batch" },
        if config::streaming() && config::live_paste() { "on" } else { "off" },
    ));
    if did_setup {
        info("Ready.\n\nPress Alt+M anywhere to dictate, and Alt+M again to paste the text.\nQuit with Ctrl+Alt+Q.");
    }

    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            hwnd,
            key,
            overlay: Overlay::new(hwnd),
            recording: None,
            stream: None,
            transcript: None,
            live: None,
            last: None,
            clipboard_later: None,
            frames: 0,
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
        WM_LIVE_TEXT => {
            with_app(|app| app.live_step());
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

fn foreground() -> HWND {
    unsafe { GetForegroundWindow() }
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
            HOTKEY_UNDO if self.recording.is_none() && !self.overlay.is_working() => self.undo(),
            _ => {}
        }
    }

    fn start(&mut self) {
        self.last = None;
        let (stream, sink) = if config::streaming() {
            let hwnd = self.hwnd as isize;
            let (stream, sink) = deepgram::Stream::start(self.key.clone(), move || unsafe {
                PostMessageW(hwnd as HWND, WM_LIVE_TEXT, 0, 0);
            });
            (Some(stream), Some(sink))
        } else {
            (None, None)
        };
        match audio::start(sink) {
            Ok(recording) => {
                self.recording = Some(recording);
                self.transcript = stream.as_ref().map(|s| s.transcript());
                self.live = (stream.is_some() && config::live_paste()).then(|| LivePaste::new(foreground()));
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
        self.transcript = None;
        let seconds = samples.len() as f32 / audio::SAMPLE_RATE as f32;
        if seconds < MIN_SECONDS {
            self.live = None;
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

    /// A new phrase arrived, or a periodic check: paste what is new.
    fn live_step(&mut self) {
        let (Some(live), Some(transcript)) = (self.live.as_mut(), self.transcript.as_ref()) else {
            return;
        };
        let hwnd = self.hwnd;
        let focused = foreground() == live.window;
        if let Some(paused) = live.step(&transcript.text(), focused, |t| paste::paste(hwnd, t)) {
            self.overlay.set_paused(paused);
        }
    }

    fn on_transcript(&mut self, result: Transcript) {
        // some text is already in the document: finish what streaming produced
        if let Some(mut live) = self.live.take().filter(|l| !l.pasted.is_empty()) {
            let text = result.unwrap_or_else(|_| live.pasted.clone());
            let hwnd = self.hwnd;
            live.step(&text, foreground() == live.window, |t| paste::paste(hwnd, t));
            self.finish_live(live, text);
            return;
        }

        match result {
            Ok(text) if text.is_empty() => {
                self.overlay.set_state(State::Notice { text: "No speech detected".into(), error: true });
            }
            Ok(text) => {
                log(&format!("-> {text}"));
                self.overlay.set_state(State::Hidden);
                let window = foreground();
                if paste::paste(self.hwnd, &text) {
                    self.remember(text.chars().count(), window);
                } else {
                    self.overlay.set_state(State::Notice { text: "Clipboard busy".into(), error: true });
                }
            }
            Err(e) => {
                log(&format!("transcription failed: {e}"));
                self.overlay.set_state(State::Notice { text: e.message(), error: true });
            }
        }
    }

    /// Wrap up a dictation that was pasted while the user spoke.
    fn finish_live(&mut self, live: LivePaste, text: String) {
        log(&format!("-> {text}"));
        self.remember(live.pasted.chars().count(), live.window);
        // the clipboard gets all of it, not the last phrase - but only after
        // the target app has read the clipboard for the last Ctrl+V
        self.clipboard_later = Some(text.clone());
        unsafe { SetTimer(self.hwnd, TIMER_CLIPBOARD, CLIPBOARD_DELAY_MS, None) };
        if live.complete(&text) {
            self.overlay.set_state(State::Hidden);
        } else {
            // stopped in another window: the rest is on the clipboard
            self.overlay.set_state(State::Notice { text: "Window changed - text copied".into(), error: false });
        }
    }

    fn remember(&mut self, chars: usize, window: HWND) {
        self.last = (chars > 0).then_some(Delivery { chars, window });
        typing::reset();
    }

    /// Delete the last dictation with Backspace. Only while the cursor is
    /// still at its end: in the same window, with no typing since.
    fn undo(&mut self) {
        let Some(last) = self.last.take() else { return };
        if typing::typed_since_reset() {
            log("undo skipped: you typed after the dictation");
            return;
        }
        if foreground() != last.window {
            log("undo skipped: a different window has focus");
            self.last = Some(last);
            return;
        }
        paste::backspaces(last.chars);
        log(&format!("undo: deleted {} characters", last.chars));
    }

    fn on_timer(&mut self, id: usize) {
        match id {
            TIMER_FRAME => {
                self.overlay.tick(audio::level());
                self.frames = self.frames.wrapping_add(1);
                if self.recording.is_some() && self.frames % LIVE_CHECK_FRAMES == 0 {
                    self.live_step();
                }
            }
            TIMER_CLIPBOARD => {
                unsafe { KillTimer(self.hwnd, TIMER_CLIPBOARD) };
                if let Some(text) = self.clipboard_later.take() {
                    paste::set_clipboard(self.hwnd, &text);
                }
            }
            _ => {}
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

pub use logging::log;

/// Save the key and the autostart choice from the setup window.
fn apply_setup(choice: &setup::Choice) -> Result<(), String> {
    let path = config::save_api_key(&choice.key)?;
    log(&format!("setup: key saved to {}", path.display()));
    if config::set_autostart(choice.autostart)? {
        log("setup: removed the Python version's Startup shortcut");
    }
    log(&format!("setup: autostart {}", if choice.autostart { "on" } else { "off" }));
    Ok(())
}

fn alert(msg: &str) {
    message(msg, MB_ICONERROR);
}

fn info(msg: &str) {
    message(msg, MB_ICONINFORMATION);
}

fn message(msg: &str, icon: MESSAGEBOX_STYLE) {
    log(msg);
    unsafe {
        MessageBoxW(ptr::null_mut(), wide(msg).as_ptr(), wide("Deepgram Dictation").as_ptr(), MB_OK | icon);
    }
}
