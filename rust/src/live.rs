//! Live paste: paste each final phrase while the user speaks.
//!
//! Text goes only into the window that had focus at the start. While
//! another window has focus, new text waits, and the meter shows "Paused".
//! It is pasted when the user comes back.

use windows_sys::Win32::Foundation::HWND;

pub struct LivePaste {
    window: HWND,
    /// The part of the transcript that is already pasted.
    pasted: String,
    paused: bool,
    broken: bool,
}

impl LivePaste {
    pub fn new(window: HWND) -> Self {
        LivePaste { window, pasted: String::new(), paused: false, broken: false }
    }

    /// Paste what is new in `text` if the start window has focus.
    /// `paste` returns false if the paste failed; then it is tried again
    /// at the next step. Returns the new paused state when it changes.
    pub fn step(&mut self, text: &str, focused: bool, mut paste: impl FnMut(&str) -> bool) -> Option<bool> {
        if self.broken {
            return None;
        }
        // the transcript only grows, so what we pasted is always its start
        if !text.starts_with(&self.pasted) {
            crate::log("live paste: transcript changed unexpectedly; stopping");
            self.broken = true;
            return None;
        }
        let new = &text[self.pasted.len()..];
        if !new.is_empty() && focused && paste(new) {
            self.pasted = text.to_string();
        }
        let paused = !focused;
        (paused != self.paused).then(|| {
            self.paused = paused;
            paused
        })
    }

    /// The window that had focus at the start.
    pub fn window(&self) -> HWND {
        self.window
    }

    /// The part of the transcript that is already pasted.
    pub fn pasted_text(&self) -> &str {
        &self.pasted
    }

    /// The number of characters already pasted.
    pub fn pasted_chars(&self) -> usize {
        self.pasted.chars().count()
    }

    /// True if all of `text` is in the start window.
    pub fn complete(&self, text: &str) -> bool {
        self.pasted == text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live() -> LivePaste {
        LivePaste::new(std::ptr::null_mut())
    }

    #[test]
    fn pastes_only_new_text() {
        let mut lp = live();
        let mut pasted = Vec::new();
        lp.step("hello", true, |t| { pasted.push(t.to_string()); true });
        lp.step("hello there", true, |t| { pasted.push(t.to_string()); true });
        lp.step("hello there", true, |t| { pasted.push(t.to_string()); true });
        assert_eq!(pasted, ["hello", " there"]);
        assert!(lp.complete("hello there"));
    }

    #[test]
    fn waits_while_another_window_has_focus() {
        let mut lp = live();
        let mut pasted = Vec::new();
        lp.step("one", true, |t| { pasted.push(t.to_string()); true });
        assert_eq!(lp.step("one two", false, |t| { pasted.push(t.to_string()); true }), Some(true));
        assert_eq!(pasted, ["one"]);
        assert_eq!(lp.step("one two", true, |t| { pasted.push(t.to_string()); true }), Some(false));
        assert_eq!(pasted, ["one", " two"]);
    }

    #[test]
    fn reports_text_left_behind() {
        let mut lp = live();
        lp.step("one", true, |_| true);
        lp.step("one two", false, |_| true);
        assert!(!lp.complete("one two"));
        assert_eq!(lp.pasted_text(), "one");
    }

    #[test]
    fn retries_a_failed_paste() {
        let mut lp = live();
        lp.step("one", true, |_| false);
        assert_eq!(lp.pasted_text(), "");
        lp.step("one", true, |_| true);
        assert_eq!(lp.pasted_text(), "one");
    }

    #[test]
    fn stops_if_the_transcript_changes() {
        let mut lp = live();
        lp.step("one", true, |_| true);
        let mut called = false;
        lp.step("two", true, |_| { called = true; true });
        lp.step("one more", true, |_| { called = true; true });
        assert!(!called);
    }

    use hegel::generators as gs;
    use hegel::{tuples, Generator, TestCase};

    /// For any run of growing transcripts, focus changes and paste
    /// results: the text that reached the window is exactly
    /// `pasted_text()`, which is always the start of the transcript.
    #[hegel::test]
    fn pasted_text_is_what_reached_the_window(tc: TestCase) {
        // each step: the text added to the transcript, whether the start
        // window has focus, and whether the paste succeeds
        let steps: Vec<(String, bool, bool)> = tc.draw(gs::vecs(tuples!(gs::text(), gs::booleans(), gs::booleans())));
        let mut lp = live();
        let mut text = String::new();
        let mut window = String::new();
        let mut paused = false;
        for (added, focused, ok) in steps {
            text.push_str(&added);
            let before = lp.pasted_text().to_string();
            let mut calls = Vec::new();
            let change = lp.step(&text, focused, |t| { calls.push(t.to_string()); if ok { window.push_str(t); } ok });
            // paste is called once, only with new text, only while focused
            let new = &text[before.len()..];
            if focused && !new.is_empty() {
                assert_eq!(calls, [new]);
            } else {
                assert!(calls.is_empty());
            }
            assert!(text.starts_with(lp.pasted_text()));
            assert_eq!(lp.pasted_text(), window);
            assert_eq!(lp.complete(&text), lp.pasted_text() == text);
            if focused && ok {
                assert!(lp.complete(&text));
            }
            // the paused state is reported only when it changes
            assert_eq!(change, (!focused != paused).then_some(!focused));
            paused = !focused;
        }
    }

    /// A transcript that does not start with the pasted text stops live
    /// paste: no later step pastes or reports anything.
    #[hegel::test]
    fn a_changed_transcript_stops_every_later_step(tc: TestCase) {
        let first: String = tc.draw(gs::text().min_size(1));
        let changed: String = tc.draw(gs::text().filter({ let first = first.clone(); move |t: &String| !t.starts_with(&first) }));
        let later: Vec<(String, bool)> = tc.draw(gs::vecs(tuples!(gs::text(), gs::booleans())));
        let mut lp = live();
        lp.step(&first, true, |_| true);
        assert!(lp.complete(&first));
        let mut called = false;
        assert_eq!(lp.step(&changed, true, |_| { called = true; true }), None);
        for (text, focused) in later {
            assert_eq!(lp.step(&text, focused, |_| { called = true; true }), None);
        }
        assert!(!called);
        assert_eq!(lp.pasted_text(), first);
    }
}
