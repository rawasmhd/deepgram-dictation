"""Unit tests for the pure helper functions in dictate.py.

These deliberately avoid the audio/GUI/hotkey machinery and only exercise the
logic that can be verified deterministically.
"""

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
