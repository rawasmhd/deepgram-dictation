"""Unit tests for the pure helper functions in dictate.py.

These deliberately avoid the audio/GUI/hotkey machinery and only exercise the
logic that can be verified deterministically.
"""

import json

import dictate


class FakeKey:
    """Minimal stand-in for a pynput key/KeyCode (only sets given attrs)."""

    def __init__(self, char=None, vk=None):
        if char is not None:
            self.char = char
        if vk is not None:
            self.vk = vk


# ---- mix() -----------------------------------------------------------------

def test_mix_endpoints():
    assert dictate.mix((0, 0, 0), (255, 255, 255), 0.0) == "#000000"
    assert dictate.mix((0, 0, 0), (255, 255, 255), 1.0) == "#ffffff"


def test_mix_midpoint():
    assert dictate.mix((0, 0, 0), (10, 10, 10), 0.5) == "#050505"


def test_mix_clamps_out_of_range():
    assert dictate.mix((0, 0, 0), (255, 255, 255), -1.0) == "#000000"
    assert dictate.mix((0, 0, 0), (255, 255, 255), 2.0) == "#ffffff"


# ---- key_matches() ---------------------------------------------------------

def test_key_matches_by_char_case_insensitive():
    assert dictate.key_matches(FakeKey(char="m"), "m")
    assert dictate.key_matches(FakeKey(char="M"), "m")


def test_key_matches_rejects_other_char():
    assert not dictate.key_matches(FakeKey(char="n"), "m")


def test_key_matches_vk_fallback_when_char_missing():
    # modifiers held -> char is None, only vk is available
    assert dictate.key_matches(FakeKey(vk=ord("M")), "m")
    assert not dictate.key_matches(FakeKey(vk=ord("N")), "m")


# ---- trigger_id() ----------------------------------------------------------

def test_trigger_id_prefers_vk():
    assert dictate.trigger_id(FakeKey(vk=77, char="m")) == 77


def test_trigger_id_falls_back_to_char():
    assert dictate.trigger_id(FakeKey(char="m")) == "m"


# ---- read_env_file() -------------------------------------------------------

def test_read_env_file_parses_quoted_value(tmp_path, monkeypatch):
    envf = tmp_path / ".env"
    envf.write_text('DEEPGRAM_API_KEY="abc123"\n', encoding="utf-8")
    monkeypatch.setattr(dictate, "ENV_FILE", envf)
    assert dictate.read_env_file() == "abc123"


def test_read_env_file_ignores_comments_and_blanks(tmp_path, monkeypatch):
    envf = tmp_path / ".env"
    envf.write_text("# comment\n\nDEEPGRAM_API_KEY=xyz\n", encoding="utf-8")
    monkeypatch.setattr(dictate, "ENV_FILE", envf)
    assert dictate.read_env_file() == "xyz"


def test_read_env_file_missing_returns_empty(tmp_path, monkeypatch):
    monkeypatch.setattr(dictate, "ENV_FILE", tmp_path / "nope.env")
    assert dictate.read_env_file() == ""


# ---- _format_transcript() --------------------------------------------------

def test_format_transcript_converts_newline_tokens():
    assert dictate._format_transcript("a<\\n\\n>b") == "a\n\nb"
    assert dictate._format_transcript("a<\\n>b") == "a\nb"


def test_format_transcript_passthrough():
    assert dictate._format_transcript("plain text") == "plain text"


# ---- StreamingSession result accumulation ----------------------------------

def _msg(transcript, is_final):
    return json.dumps({
        "channel": {"alternatives": [{"transcript": transcript}]},
        "is_final": is_final,
    })


def test_streaming_accumulates_only_finals():
    s = dictate.StreamingSession("key")
    s._on_message(None, _msg("hello", True))
    s._on_message(None, _msg("world", False))   # interim -> ignored
    s._on_message(None, _msg("there", True))
    assert s.transcript() == "hello there"


def test_streaming_transcript_empty_by_default():
    s = dictate.StreamingSession("key")
    assert s.transcript() == ""


def test_streaming_ignores_malformed_messages():
    s = dictate.StreamingSession("key")
    s._on_message(None, "not json")
    s._on_message(None, _msg("ok", True))
    assert s.transcript() == "ok"


def test_streaming_calls_on_final_for_finals_only():
    calls = []
    s = dictate.StreamingSession("key", on_final=lambda: calls.append(1))
    s._on_message(None, _msg("hello", False))
    s._on_message(None, _msg("hello", True))
    assert calls == [1]


# ---- LivePaster ------------------------------------------------------------

def _live(monkeypatch, window="A"):
    """A LivePaster over a real session, with paste and focus faked."""
    focus = {"window": window}
    pasted = []
    monkeypatch.setattr(dictate, "foreground_window", lambda: focus["window"])
    monkeypatch.setattr(dictate, "paste", pasted.append)
    session = dictate.StreamingSession("key")
    return dictate.LivePaster(session), session, focus, pasted


def test_live_paste_sends_only_new_text(monkeypatch):
    paster, session, _, pasted = _live(monkeypatch)
    session._on_message(None, _msg("hello", True))
    paster._step()
    session._on_message(None, _msg("there", True))
    paster._step()
    paster._step()                              # nothing new -> no paste
    assert pasted == ["hello", " there"]
    assert paster.finish() is True


def test_live_paste_waits_while_another_window_has_focus(monkeypatch):
    paster, session, focus, pasted = _live(monkeypatch)
    session._on_message(None, _msg("one", True))
    paster._step()
    focus["window"] = "B"
    session._on_message(None, _msg("two", True))
    paster._step()
    assert pasted == ["one"] and paster.paused
    focus["window"] = "A"
    paster._step()
    assert pasted == ["one", " two"] and not paster.paused


def test_live_paste_finish_reports_text_left_behind(monkeypatch):
    paster, session, focus, pasted = _live(monkeypatch)
    session._on_message(None, _msg("one", True))
    paster._step()
    focus["window"] = "B"
    session._on_message(None, _msg("two", True))
    assert paster.finish() is False
    assert paster.pasted == "one"


# ---- undo ------------------------------------------------------------------

def test_undo_sends_one_backspace_per_character(monkeypatch):
    sent = []
    monkeypatch.setattr(dictate, "foreground_window", lambda: "A")
    monkeypatch.setattr(dictate, "_send_keys", sent.append)
    dictate.remember_delivery("hi\nyou", "A")
    dictate.undo_last()
    assert len(sent) == 1 and dictate._last_delivery is None

    class Kbd:
        presses = 0

        def press(self, key):
            Kbd.presses += 1

        def release(self, key):
            pass
    sent[0](Kbd())
    assert Kbd.presses == 6


def test_undo_skipped_in_another_window(monkeypatch):
    sent = []
    monkeypatch.setattr(dictate, "foreground_window", lambda: "B")
    monkeypatch.setattr(dictate, "_send_keys", sent.append)
    dictate.remember_delivery("hello", "A")
    dictate.undo_last()
    assert sent == [] and dictate._last_delivery is not None
    dictate.forget_delivery()


def test_typing_cancels_undo(monkeypatch):
    monkeypatch.setattr(dictate, "_synthetic_until", 0.0)
    dictate.remember_delivery("hello", "A")
    dictate.on_press(FakeKey(char="x", vk=ord("X")))
    dictate.on_release(FakeKey(char="x", vk=ord("X")))
    assert dictate._last_delivery is None
