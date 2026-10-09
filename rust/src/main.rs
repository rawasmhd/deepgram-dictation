//! Deepgram dictation, Rust rewrite.
//!
//!   Alt+M        start dictating
//!   Alt+M        stop, transcribe, paste at the cursor
//!   Ctrl+Alt+Z   delete the last dictation, if you have not typed since
//!   Ctrl+Alt+Q   quit
//!
//! The first start, or `dictation.exe --setup`, asks for the API key.
//! The tray icon shows the state and has a menu: settings, autostart, the
//! log, quit.
//!
//! This file has the Win32 message loop, the hotkeys and the tray menu.
//! The dictation itself is in `session`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod art;
mod audio;
mod config;
mod deepgram;
mod gfx;
mod live;
mod logging;
mod overlay;
mod paste;
mod session;
mod setup;
mod theme;
mod tray;
mod typing;
mod words;
mod words_window;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::{cell::RefCell, ptr, thread};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use art::Dot;
use overlay::{wide, Overlay, State, TIMER_FRAME};
use session::{Host, Session, Transcript};
use tray::{Tray, WM_TRAY};
use words::Words;

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

/// The same name as the old Python version (dictate.py), so an old copy and
/// this one never run at the same time.
const INSTANCE_MUTEX: &str = "DeepgramDictation_v1";

/// Explorer's "TaskbarCreated" message, to add the tray icon again.
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(u32::MAX);
/// The settings window is open (from the tray menu).
static SETTINGS_OPEN: AtomicBool = AtomicBool::new(false);

struct App {
    session: Session<Desktop>,
    frames: u32,
}

/// The real `session::Host`: the microphone, Deepgram, the Win32 windows,
/// the meter and the tray.
struct Desktop {
    hwnd: HWND,
    key: String,
    overlay: Overlay,
    tray: Tray,
    /// The text for the clipboard when `TIMER_CLIPBOARD` fires.
    clipboard_later: Option<String>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn main() {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    logging::init();

    // only the Custom words window, without the app
    if std::env::args().any(|a| a == "--words") {
        words_window::open(&Words::new());
        return;
    }

    // setup: on the first start (no key), or when asked with --setup
    let asked = std::env::args().any(|a| a == "--setup");
    let mut key = config::api_key();
    let mut did_setup = false;
    if asked || key.is_none() {
        match setup::run(key.as_deref(), key.is_none() || config::autostart_enabled(), key.is_none()) {
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
    theme::allow_dark_menus();
    tray::track_foreground();
    TASKBAR_CREATED.store(tray::taskbar_created_message(), Ordering::Relaxed);
    let tray = Tray::add(hwnd);
    if did_setup {
        tray.balloon("Ready", "Press Alt+M anywhere to dictate, and Alt+M again to paste the text.");
    }

    let desktop = Desktop { hwnd, key, overlay: Overlay::new(hwnd), tray, clipboard_later: None };
    APP.with(|a| *a.borrow_mut() = Some(App { session: Session::new(desktop, Words::new()), frames: 0 }));

    register_restart();

    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    quit();
}

/// Drop the app state, which stops the microphone and removes the tray
/// icon. Safe to call twice: Windows can end the process right after
/// WM_ENDSESSION, before the message loop returns.
fn quit() {
    if APP.with(|a| a.borrow_mut().take()).is_some() {
        log("stopped");
    }
}

/// Windows closes the app: the user signs out, or Restart Manager closes
/// it for an update. The second case happens when the app was started
/// from inside another app, for example a Claude Code session, and that
/// app updates itself (#47). Quit cleanly, so Windows does not kill the
/// process and report a hang.
fn on_end_session(lp: LPARAM) {
    let why = if lp as u32 & ENDSESSION_CLOSEAPP != 0 { "an app update (Restart Manager)" } else { "sign out or shutdown" };
    log(&format!("closed by Windows: {why}"));
    with_app(|app| app.on_hotkey(HOTKEY_QUIT));
    quit();
}

/// Ask Restart Manager to start the app again after it closed it for an
/// update (#47). Not after a crash, a hang, or a reboot: the autostart
/// entry covers the reboot.
fn register_restart() {
    const RESTART_NO_CRASH: u32 = 1;
    const RESTART_NO_HANG: u32 = 2;
    const RESTART_NO_REBOOT: u32 = 8;
    #[link(name = "kernel32")]
    extern "system" {
        fn RegisterApplicationRestart(command_line: *const u16, flags: u32) -> i32;
    }
    let hr = unsafe { RegisterApplicationRestart(ptr::null(), RESTART_NO_CRASH | RESTART_NO_HANG | RESTART_NO_REBOOT) };
    if hr < 0 {
        log(&format!("restart after an update not registered: 0x{hr:08x}"));
    }
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
            with_app(|app| app.session.live_step());
            0
        }
        WM_TRANSCRIPT => {
            // the worker thread gave up ownership of this box
            let result = *Box::from_raw(lp as *mut Transcript);
            with_app(|app| app.session.on_transcript(result));
            0
        }
        WM_TRAY => {
            let event = (lp & 0xFFFF) as u32;
            if event == WM_LBUTTONUP || event == WM_RBUTTONUP {
                on_tray_menu(hwnd);
            }
            0
        }
        WM_SETTINGCHANGE => {
            // light or dark mode may have changed
            with_app(|app| app.desktop().tray.theme_changed());
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        // DefWindowProc answers WM_QUERYENDSESSION with "yes"; this is the
        // confirmation that the session, or the app, ends now
        WM_ENDSESSION => {
            if wp != 0 {
                on_end_session(lp);
            }
            0
        }
        m if m == TASKBAR_CREATED.load(Ordering::Relaxed) => {
            with_app(|app| app.desktop().tray.add_again());
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
                app.sync_tray();
            }
        }
    });
}

/// The tray menu. It runs its own message loop, so the app is not
/// borrowed while it is open.
fn on_tray_menu(hwnd: HWND) {
    let mut state = None;
    with_app(|app| state = Some((app.session.recording(), app.session.host().overlay.is_working(), app.status().1)));
    let Some((recording, busy, status)) = state else { return };
    let menu = tray::MenuState { status: &status, recording, busy, autostart: config::autostart_enabled() };
    match tray::menu(hwnd, &menu) {
        Some(tray::CMD_TOGGLE) => {
            // back to the window the user worked in, so the text goes there
            tray::restore_focus();
            with_app(|app| app.on_hotkey(HOTKEY_TOGGLE));
        }
        Some(tray::CMD_SETTINGS) => open_settings(),
        Some(tray::CMD_WORDS) => {
            let mut words = None;
            with_app(|app| words = Some(app.session.words().clone()));
            if let Some(words) = words {
                words_window::open(&words);
            }
        }
        Some(tray::CMD_AUTOSTART) => {
            let on = !config::autostart_enabled();
            match config::set_autostart(on) {
                Ok(_) => log(&format!("tray: autostart {}", if on { "on" } else { "off" })),
                Err(e) => alert(&format!("Could not change the autostart.\n\n{e}")),
            }
        }
        Some(tray::CMD_LOG) => {
            if let Some(path) = logging::path() {
                let path = wide(&path.display().to_string());
                unsafe { ShellExecuteW(hwnd, wide("open").as_ptr(), path.as_ptr(), ptr::null(), ptr::null(), SW_SHOWNORMAL) };
            }
        }
        Some(tray::CMD_QUIT) => with_app(|app| app.on_hotkey(HOTKEY_QUIT)),
        _ => {}
    }
}

/// The setup window, from the tray menu. A new key is used at once.
fn open_settings() {
    if SETTINGS_OPEN.swap(true, Ordering::Relaxed) {
        return;
    }
    let mut key = String::new();
    with_app(|app| key = app.desktop().key.clone());
    if let Some(choice) = setup::run(Some(&key), config::autostart_enabled(), false) {
        match apply_setup(&choice) {
            Ok(()) => with_app(|app| app.desktop().key = choice.key),
            Err(e) => alert(&format!("The settings could not be saved.\n\n{e}")),
        }
    }
    SETTINGS_OPEN.store(false, Ordering::Relaxed);
}

impl App {
    fn desktop(&mut self) -> &mut Desktop {
        self.session.host_mut()
    }

    /// The tray icon's dot, and a line for the tooltip and the menu.
    fn status(&self) -> (Dot, String) {
        match self.session.host().overlay.state() {
            _ if self.session.recording() => (Dot::Recording, "Recording… (Alt+M to stop)".into()),
            _ if self.session.working() => (Dot::Working, "Transcribing…".into()),
            State::Notice { text, error: true } => (Dot::Problem, text.clone()),
            _ => (Dot::None, "Ready. Press Alt+M to dictate.".into()),
        }
    }

    fn sync_tray(&mut self) {
        let (dot, line) = self.status();
        let tip = if dot == Dot::None { tray::TIP_READY.to_string() } else { line };
        self.desktop().tray.set(dot, &tip);
    }

    fn on_hotkey(&mut self, id: i32) {
        match id {
            // the message loop ends, and `quit` drops the session: that
            // stops the microphone and the stream
            HOTKEY_QUIT => unsafe { PostQuitMessage(0) },
            HOTKEY_TOGGLE => self.session.toggle(),
            HOTKEY_UNDO => self.session.undo(),
            _ => {}
        }
    }

    fn on_timer(&mut self, id: usize) {
        match id {
            TIMER_FRAME => {
                self.desktop().overlay.tick(audio::level());
                self.frames = self.frames.wrapping_add(1);
                if self.session.recording() && self.frames % LIVE_CHECK_FRAMES == 0 {
                    self.session.live_step();
                }
            }
            TIMER_CLIPBOARD => self.desktop().clipboard_now(),
            _ => {}
        }
    }
}

impl Desktop {
    /// `TIMER_CLIPBOARD` fired: the target app has read the last paste.
    fn clipboard_now(&mut self) {
        unsafe { KillTimer(self.hwnd, TIMER_CLIPBOARD) };
        if let Some(text) = self.clipboard_later.take() {
            paste::set_clipboard(self.hwnd, &text);
        }
    }
}

impl Host for Desktop {
    type Stream = deepgram::Stream;

    fn record(&mut self, sink: Option<Sender<Vec<i16>>>) -> Result<Box<dyn session::Recording>, String> {
        audio::start(sink).map(|r| Box::new(r) as Box<dyn session::Recording>)
    }

    fn stream(&mut self, terms: Vec<String>, words: Words) -> Option<(deepgram::Stream, Sender<Vec<i16>>)> {
        if !config::streaming() {
            return None;
        }
        let hwnd = self.hwnd as isize;
        Some(deepgram::Stream::start(self.key.clone(), terms, words, move || unsafe {
            PostMessageW(hwnd as HWND, WM_LIVE_TEXT, 0, 0);
        }))
    }

    fn live_paste(&self) -> bool {
        config::live_paste()
    }

    /// Wait for Deepgram off the UI thread, so the meter keeps moving.
    fn finish(&mut self, stream: Option<deepgram::Stream>, samples: Vec<i16>, terms: Vec<String>, words: Words) {
        let key = self.key.clone();
        let hwnd = self.hwnd as isize;
        thread::spawn(move || {
            let result = deepgram::finish(&key, stream, &samples, &terms, &words);
            let boxed = Box::into_raw(Box::new(result)) as isize;
            unsafe {
                if PostMessageW(hwnd as HWND, WM_TRANSCRIPT, 0, boxed) == 0 {
                    drop(Box::from_raw(boxed as *mut Transcript));
                }
            }
        });
    }

    fn foreground(&self) -> HWND {
        unsafe { GetForegroundWindow() }
    }

    fn paste(&mut self, text: &str) -> bool {
        paste::paste(self.hwnd, text)
    }

    fn backspaces(&mut self, count: usize) {
        paste::backspaces(count)
    }

    fn typed_since_reset(&self) -> bool {
        typing::typed_since_reset()
    }

    fn reset_typing(&mut self) {
        typing::reset()
    }

    /// Only after the target app has read the clipboard for the last Ctrl+V.
    fn clipboard_later(&mut self, text: String) {
        self.clipboard_later = Some(text);
        unsafe { SetTimer(self.hwnd, TIMER_CLIPBOARD, CLIPBOARD_DELAY_MS, None) };
    }

    fn show(&mut self, state: State) {
        self.overlay.set_state(state)
    }

    fn set_paused(&mut self, paused: bool) {
        self.overlay.set_paused(paused)
    }

    fn balloon(&mut self, title: &str, text: &str) {
        self.tray.balloon(title, text)
    }
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
