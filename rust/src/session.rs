//! The dictation: from Alt+M to the text at the cursor.
//!
//! One `Session` holds the state machine: idle, recording, transcribing,
//! and the last dictation for undo. It does not touch the microphone,
//! Deepgram, the target window or the meter itself. The `Host` does that,
//! so the tests run the whole flow with fakes.

use std::mem;
use std::sync::mpsc::Sender;

use windows_sys::Win32::Foundation::HWND;

use crate::audio::{self, SAMPLE_RATE};
use crate::deepgram;
use crate::live::LivePaste;
use crate::log;
use crate::overlay::State;
use crate::words::Words;

const MIN_SECONDS: f32 = 0.4; // ignore accidental taps

/// The text of a dictation, or why there is none.
pub type Transcript = Result<String, deepgram::Error>;

/// A running recording. `stop` ends it and returns the samples.
pub trait Recording {
    fn stop(self: Box<Self>) -> Vec<i16>;
}

impl Recording for audio::Recording {
    fn stop(self: Box<Self>) -> Vec<i16> {
        audio::Recording::stop(*self)
    }
}

/// A live transcription, read while the user speaks.
pub trait Live {
    /// The final text so far.
    fn text(&self) -> String;
}

impl Live for deepgram::Stream {
    fn text(&self) -> String {
        self.transcript().text()
    }
}

/// What the session needs from the outside. `main` gives the real
/// microphone, Deepgram, windows and meter; the tests give fakes.
pub trait Host {
    /// A live transcription: `deepgram::Stream`, or a fake.
    type Stream: Live;

    // -- the microphone and Deepgram

    /// Start the microphone. The audio also goes to `sink`, if there is one.
    fn record(&mut self, sink: Option<Sender<Vec<i16>>>) -> Result<Box<dyn Recording>, String>;
    /// Open a live transcription, if streaming is on. The host calls
    /// `Session::live_step` after each new phrase.
    fn stream(&mut self, terms: Vec<String>, words: Words) -> Option<(Self::Stream, Sender<Vec<i16>>)>;
    /// Paste each phrase while the user speaks (streaming only).
    fn live_paste(&self) -> bool;
    /// Get the text of the recording off the UI thread, then give it to
    /// `Session::on_transcript`.
    fn finish(&mut self, stream: Option<Self::Stream>, samples: Vec<i16>, terms: Vec<String>, words: Words);

    // -- the target window

    /// The window that has focus.
    fn foreground(&self) -> HWND;
    /// Paste at the cursor. False if the clipboard was busy.
    fn paste(&mut self, text: &str) -> bool;
    /// Delete `count` characters before the cursor.
    fn backspaces(&mut self, count: usize);
    /// The user pressed a key since `reset_typing`.
    fn typed_since_reset(&self) -> bool;
    fn reset_typing(&mut self);
    /// Put the text on the clipboard, after the target app has read the
    /// clipboard for the last paste.
    fn clipboard_later(&mut self, text: String);

    // -- the meter and the tray

    fn show(&mut self, state: State);
    /// Live paste waits for the focus to come back.
    fn set_paused(&mut self, paused: bool);
    fn balloon(&mut self, title: &str, text: &str);
}

/// The last dictation, for undo: how many characters, and where.
struct Delivery {
    chars: usize,
    window: HWND,
}

enum Phase<S> {
    Idle,
    Recording { recording: Box<dyn Recording>, stream: Option<S>, terms: Vec<String>, live: Option<LivePaste> },
    /// Waiting for the transcript. `live` has what was pasted while the
    /// user spoke.
    Working { live: Option<LivePaste> },
}

pub struct Session<H: Host> {
    // the devices stop before the host goes (drop order)
    phase: Phase<H::Stream>,
    last: Option<Delivery>,
    /// The custom words for each dictation.
    words: Words,
    host: H,
}

impl<H: Host> Session<H> {
    pub fn new(host: H, words: Words) -> Self {
        Session { phase: Phase::Idle, last: None, words, host }
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut H {
        &mut self.host
    }

    pub fn words(&self) -> &Words {
        &self.words
    }

    /// The microphone is on.
    pub fn recording(&self) -> bool {
        matches!(self.phase, Phase::Recording { .. })
    }

    /// Waiting for the transcript.
    pub fn working(&self) -> bool {
        matches!(self.phase, Phase::Working { .. })
    }

    /// Alt+M: start, or stop. Ignored while transcribing.
    pub fn toggle(&mut self) {
        match self.phase {
            Phase::Idle => self.start(),
            Phase::Recording { .. } => self.stop(),
            Phase::Working { .. } => {}
        }
    }

    /// Start recording. Ignored unless idle.
    pub fn start(&mut self) {
        if !matches!(self.phase, Phase::Idle) {
            return;
        }
        self.last = None;
        let terms = self.words.for_request();
        let (stream, sink) = match self.host.stream(terms.clone(), self.words.clone()) {
            Some((stream, sink)) => (Some(stream), Some(sink)),
            None => (None, None),
        };
        match self.host.record(sink) {
            Ok(recording) => {
                let live = (stream.is_some() && self.host.live_paste()).then(|| LivePaste::new(self.host.foreground()));
                self.phase = Phase::Recording { recording, stream, terms, live };
                self.host.show(State::Recording);
            }
            Err(e) => {
                log(&format!("microphone unavailable: {e}"));
                self.host.show(State::Notice { text: "Microphone unavailable".into(), error: true });
            }
        }
    }

    /// Stop recording and ask for the transcript. Ignored unless recording.
    pub fn stop(&mut self) {
        let (recording, stream, terms, live) = match mem::replace(&mut self.phase, Phase::Idle) {
            Phase::Recording { recording, stream, terms, live } => (recording, stream, terms, live),
            other => {
                self.phase = other;
                return;
            }
        };
        // stopping closes the audio channel, so the stream starts to finish
        let samples = recording.stop();
        let seconds = samples.len() as f32 / SAMPLE_RATE as f32;
        if seconds < MIN_SECONDS {
            self.host.show(State::Hidden);
            return;
        }
        self.phase = Phase::Working { live };
        self.host.show(State::Working);
        self.host.finish(stream, samples, terms, self.words.clone());
    }

    /// A new phrase arrived, or a periodic check: paste what is new.
    pub fn live_step(&mut self) {
        let Phase::Recording { stream: Some(stream), live: Some(live), .. } = &mut self.phase else {
            return;
        };
        let text = stream.text();
        let focused = self.host.foreground() == live.window();
        let host = &mut self.host;
        if let Some(paused) = live.step(&text, focused, |t| host.paste(t)) {
            host.set_paused(paused);
        }
    }

    /// The transcript is ready: paste it, or finish the live paste.
    pub fn on_transcript(&mut self, result: Transcript) {
        let live = match mem::replace(&mut self.phase, Phase::Idle) {
            Phase::Working { live } => live,
            other => {
                self.phase = other;
                log("transcript arrived while not transcribing; ignored");
                return;
            }
        };
        if self.words.take_notice() {
            self.host.balloon("Custom words not used", "Your custom words list is too long. Remove some words from words.txt.");
        }
        // some text is already in the document: finish what streaming produced
        if let Some(mut live) = live.filter(|l| l.pasted_chars() > 0) {
            let text = result.unwrap_or_else(|_| live.pasted_text().to_string());
            let focused = self.host.foreground() == live.window();
            let host = &mut self.host;
            live.step(&text, focused, |t| host.paste(t));
            self.finish_live(live, text);
            return;
        }

        match result {
            Ok(text) if text.is_empty() => {
                self.host.show(State::Notice { text: "No speech detected".into(), error: true });
            }
            Ok(text) => {
                log(&format!("-> {text}"));
                self.host.show(State::Hidden);
                let window = self.host.foreground();
                if self.host.paste(&text) {
                    self.remember(text.chars().count(), window);
                } else {
                    self.host.show(State::Notice { text: "Clipboard busy".into(), error: true });
                }
            }
            Err(e) => {
                log(&format!("transcription failed: {e}"));
                self.host.show(State::Notice { text: e.message(), error: true });
            }
        }
    }

    /// Wrap up a dictation that was pasted while the user spoke.
    fn finish_live(&mut self, live: LivePaste, text: String) {
        log(&format!("-> {text}"));
        self.remember(live.pasted_chars(), live.window());
        // the clipboard gets all of it, not the last phrase
        self.host.clipboard_later(text.clone());
        if live.complete(&text) {
            self.host.show(State::Hidden);
        } else {
            // stopped in another window: the rest is on the clipboard
            self.host.show(State::Notice { text: "Window changed - text copied".into(), error: false });
        }
    }

    fn remember(&mut self, chars: usize, window: HWND) {
        self.last = (chars > 0).then_some(Delivery { chars, window });
        self.host.reset_typing();
    }

    /// Delete the last dictation with Backspace. Only while the cursor is
    /// still at its end: in the same window, with no typing since. Ignored
    /// while recording or transcribing.
    pub fn undo(&mut self) {
        if !matches!(self.phase, Phase::Idle) {
            return;
        }
        let Some(last) = self.last.take() else { return };
        if self.host.typed_since_reset() {
            log("undo skipped: you typed after the dictation");
            return;
        }
        if self.host.foreground() != last.window {
            log("undo skipped: a different window has focus");
            self.last = Some(last);
            return;
        }
        self.host.backspaces(last.chars);
        log(&format!("undo: deleted {} characters", last.chars));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    const EDITOR: HWND = 1 as HWND;
    const OTHER: HWND = 2 as HWND;

    struct FakeRecording(Vec<i16>);

    impl Recording for FakeRecording {
        fn stop(self: Box<Self>) -> Vec<i16> {
            self.0
        }
    }

    /// The live text, shared with the fake host so a test can add to it.
    struct FakeStream(Rc<RefCell<String>>);

    impl Live for FakeStream {
        fn text(&self) -> String {
            self.0.borrow().clone()
        }
    }

    struct Fake {
        streaming: bool,
        live_paste: bool,
        /// The length of the next recording.
        seconds: f32,
        mic_ok: bool,
        focus: HWND,
        typed: bool,
        clipboard_ok: bool,
        live: Rc<RefCell<String>>,
        // what happened
        pasted: Vec<String>,
        deleted: Vec<usize>,
        finished: Vec<Vec<i16>>,
        states: Vec<State>,
        paused: Vec<bool>,
        clipboard: Option<String>,
        balloons: Vec<String>,
    }

    impl Fake {
        fn new() -> Fake {
            Fake {
                streaming: false,
                live_paste: false,
                seconds: 1.0,
                mic_ok: true,
                focus: EDITOR,
                typed: false,
                clipboard_ok: true,
                live: Rc::default(),
                pasted: Vec::new(),
                deleted: Vec::new(),
                finished: Vec::new(),
                states: Vec::new(),
                paused: Vec::new(),
                clipboard: None,
                balloons: Vec::new(),
            }
        }

        fn live() -> Fake {
            Fake { streaming: true, live_paste: true, ..Fake::new() }
        }

        fn speak(&self, text: &str) {
            *self.live.borrow_mut() = text.to_string();
        }
    }

    impl Host for Fake {
        type Stream = FakeStream;

        fn record(&mut self, _sink: Option<Sender<Vec<i16>>>) -> Result<Box<dyn Recording>, String> {
            if !self.mic_ok {
                return Err("no device".into());
            }
            Ok(Box::new(FakeRecording(vec![0; (self.seconds * SAMPLE_RATE as f32) as usize])))
        }

        fn stream(&mut self, _terms: Vec<String>, _words: Words) -> Option<(FakeStream, Sender<Vec<i16>>)> {
            self.streaming.then(|| (FakeStream(self.live.clone()), std::sync::mpsc::channel().0))
        }

        fn live_paste(&self) -> bool {
            self.live_paste
        }

        fn finish(&mut self, _stream: Option<FakeStream>, samples: Vec<i16>, _terms: Vec<String>, _words: Words) {
            self.finished.push(samples);
        }

        fn foreground(&self) -> HWND {
            self.focus
        }

        fn paste(&mut self, text: &str) -> bool {
            if self.clipboard_ok {
                self.pasted.push(text.to_string());
            }
            self.clipboard_ok
        }

        fn backspaces(&mut self, count: usize) {
            self.deleted.push(count);
        }

        fn typed_since_reset(&self) -> bool {
            self.typed
        }

        fn reset_typing(&mut self) {
            self.typed = false;
        }

        fn clipboard_later(&mut self, text: String) {
            self.clipboard = Some(text);
        }

        fn show(&mut self, state: State) {
            self.states.push(state);
        }

        fn set_paused(&mut self, paused: bool) {
            self.paused.push(paused);
        }

        fn balloon(&mut self, title: &str, _text: &str) {
            self.balloons.push(title.to_string());
        }
    }

    fn session(host: Fake) -> Session<Fake> {
        Session::new(host, Words::new())
    }

    /// Record, stop, and receive `text`: a whole batch dictation.
    fn dictate(s: &mut Session<Fake>, text: &str) {
        s.start();
        s.stop();
        s.on_transcript(Ok(text.to_string()));
    }

    fn hidden(state: &State) -> bool {
        matches!(state, State::Hidden)
    }

    fn error(state: &State) -> bool {
        matches!(state, State::Notice { error: true, .. })
    }

    #[test]
    fn a_short_tap_is_ignored() {
        let mut s = session(Fake { seconds: 0.2, ..Fake::new() });
        s.start();
        assert!(s.recording());
        s.stop();
        assert!(!s.recording() && !s.working());
        assert!(s.host().finished.is_empty());
        assert!(hidden(s.host().states.last().unwrap()));
    }

    #[test]
    fn a_transcript_is_pasted_and_remembered() {
        let mut s = session(Fake::new());
        s.start();
        s.stop();
        assert!(s.working());
        assert_eq!(s.host().finished.len(), 1);
        assert_eq!(s.host().finished[0].len(), SAMPLE_RATE as usize);
        s.on_transcript(Ok("hello".into()));
        assert!(!s.working());
        assert_eq!(s.host().pasted, ["hello"]);
        assert!(hidden(s.host().states.last().unwrap()));
        assert_eq!(s.last.as_ref().unwrap().chars, 5);
        assert_eq!(s.last.as_ref().unwrap().window, EDITOR);
    }

    #[test]
    fn undo_in_the_same_window_deletes() {
        let mut s = session(Fake::new());
        dictate(&mut s, "héllo");
        s.undo();
        assert_eq!(s.host().deleted, [5]);
        // only once
        s.undo();
        assert_eq!(s.host().deleted, [5]);
    }

    #[test]
    fn undo_after_typing_does_nothing() {
        let mut s = session(Fake::new());
        dictate(&mut s, "hello");
        s.host_mut().typed = true;
        s.undo();
        assert!(s.host().deleted.is_empty());
        // and the dictation is forgotten
        s.host_mut().typed = false;
        s.undo();
        assert!(s.host().deleted.is_empty());
    }

    #[test]
    fn undo_in_another_window_does_nothing() {
        let mut s = session(Fake::new());
        dictate(&mut s, "hello");
        s.host_mut().focus = OTHER;
        s.undo();
        assert!(s.host().deleted.is_empty());
        // back in the window, undo still works
        s.host_mut().focus = EDITOR;
        s.undo();
        assert_eq!(s.host().deleted, [5]);
    }

    #[test]
    fn undo_waits_for_the_transcript() {
        let mut s = session(Fake::new());
        dictate(&mut s, "hello");
        s.start();
        s.undo();
        s.stop();
        s.undo();
        assert!(s.host().deleted.is_empty());
    }

    #[test]
    fn live_text_already_pasted_is_finished_not_pasted_twice() {
        let mut s = session(Fake::live());
        s.start();
        s.host().speak("hello");
        s.live_step();
        s.host().speak("hello there");
        s.live_step();
        s.live_step();
        assert_eq!(s.host().pasted, ["hello", " there"]);
        s.stop();
        s.on_transcript(Ok("hello there".into()));
        assert_eq!(s.host().pasted, ["hello", " there"]);
        assert_eq!(s.host().clipboard.as_deref(), Some("hello there"));
        assert!(hidden(s.host().states.last().unwrap()));
        s.undo();
        assert_eq!(s.host().deleted, [11]);
    }

    #[test]
    fn live_text_waits_while_another_window_has_focus() {
        let mut s = session(Fake::live());
        s.start();
        s.host().speak("one");
        s.live_step();
        s.host_mut().focus = OTHER;
        s.host().speak("one two");
        s.live_step();
        assert_eq!(s.host().paused, [true]);
        assert_eq!(s.host().pasted, ["one"]);
        s.stop();
        s.on_transcript(Ok("one two".into()));
        // the rest is on the clipboard, and the meter says so
        assert_eq!(s.host().pasted, ["one"]);
        assert_eq!(s.host().clipboard.as_deref(), Some("one two"));
        assert!(matches!(s.host().states.last().unwrap(), State::Notice { error: false, .. }));
        // undo deletes only what was pasted, in the start window
        s.host_mut().focus = EDITOR;
        s.undo();
        assert_eq!(s.host().deleted, [3]);
    }

    #[test]
    fn live_text_is_kept_when_the_transcript_fails() {
        let mut s = session(Fake::live());
        s.start();
        s.host().speak("hello");
        s.live_step();
        s.stop();
        s.on_transcript(Err(deepgram::Error::Network("down".into())));
        assert_eq!(s.host().pasted, ["hello"]);
        assert_eq!(s.host().clipboard.as_deref(), Some("hello"));
        assert!(hidden(s.host().states.last().unwrap()));
    }

    #[test]
    fn nothing_pasted_live_falls_back_to_one_paste() {
        let mut s = session(Fake::live());
        s.start();
        s.stop();
        s.on_transcript(Ok("hello".into()));
        assert_eq!(s.host().pasted, ["hello"]);
        assert!(s.host().clipboard.is_none());
    }

    #[test]
    fn problems_show_a_notice() {
        let mut s = session(Fake::new());
        dictate(&mut s, "");
        assert!(error(s.host().states.last().unwrap()));
        assert!(s.host().pasted.is_empty());

        s.start();
        s.stop();
        s.on_transcript(Err(deepgram::Error::KeyRejected));
        assert!(error(s.host().states.last().unwrap()));

        s.host_mut().clipboard_ok = false;
        dictate(&mut s, "hello");
        assert!(error(s.host().states.last().unwrap()));
        s.undo();
        assert!(s.host().deleted.is_empty());

        let mut s = session(Fake { mic_ok: false, ..Fake::new() });
        s.start();
        assert!(!s.recording());
        assert!(error(s.host().states.last().unwrap()));
    }

    #[test]
    fn toggle_is_ignored_while_transcribing() {
        let mut s = session(Fake::new());
        s.toggle();
        assert!(s.recording());
        s.toggle();
        assert!(s.working());
        s.toggle();
        assert!(s.working());
        assert_eq!(s.host().finished.len(), 1);
        s.on_transcript(Ok("hello".into()));
        s.toggle();
        assert!(s.recording());
    }
}
