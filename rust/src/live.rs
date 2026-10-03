//! Live paste: paste each final phrase while the user speaks.
//!
//! Text goes only into the window that had focus at the start. While
//! another window has focus, new text waits, and the meter shows "Paused".
//! It is pasted when the user comes back.

use windows_sys::Win32::Foundation::HWND;

pub struct LivePaste {
    pub window: HWND,
    /// The part of the transcript that is already pasted.
    pub pasted: String,
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
        assert_eq!(lp.pasted, "one");
    }

    #[test]
    fn retries_a_failed_paste() {
        let mut lp = live();
        lp.step("one", true, |_| false);
        assert_eq!(lp.pasted, "");
        lp.step("one", true, |_| true);
        assert_eq!(lp.pasted, "one");
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
}
