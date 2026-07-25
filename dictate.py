#!/usr/bin/env python3
"""
Deepgram dictation with a floating microphone meter.

  Alt+M        start dictating
  Alt+M        stop, transcribe, paste at the cursor
  Ctrl+Alt+Q   quit

Runs silently in the background. The only thing you see is a small
level meter that floats just above the taskbar while recording.
"""

import io
import os
import sys
import math
import time
import wave
import queue
import ctypes
import threading
import platform
from pathlib import Path
from collections import deque

import numpy as np
import requests
import sounddevice as sd
import pyperclip
from pynput import keyboard

# ----------------------------------------------------------------------------
# Config
# ----------------------------------------------------------------------------

HOTKEY_MODIFIERS = {"alt"}          # combination of: alt, ctrl, shift, win
HOTKEY_CHAR = "m"

QUIT_MODIFIERS = {"ctrl", "alt"}
QUIT_CHAR = "q"

AUTO_PASTE = True                   # False = copy to clipboard only
RESTORE_CLIPBOARD = True
CLIPBOARD_RESTORE_DELAY = 0.8       # seconds to let the paste land first
BEEP = False                        # short tones on start / stop / error
MIN_SECONDS = 0.4                   # ignore accidental taps

SHOW_OVERLAY = True
OVERLAY_POSITION = "taskbar"        # center | top | bottom | taskbar
OVERLAY_MARGIN = 12                 # gap above the taskbar, in pixels

# Deepgram query parameters
# docs: developers.deepgram.com/docs/pre-recorded-audio
DG_PARAMS = {
    "model": "nova-3",
    "language": "en",
    "smart_format": "true",
    "punctuate": "true",
    "dictation": "true",            # spoken "comma" -> ","
    "filler_words": "false",
}

ENDPOINT = "https://api.deepgram.com/v1/listen"
SAMPLE_RATE = 16_000
CHANNELS = 1
SAMPLE_WIDTH = 2

IS_WINDOWS = platform.system() == "Windows"
IS_MAC = platform.system() == "Darwin"
PASTE_MODIFIER = keyboard.Key.cmd if IS_MAC else keyboard.Key.ctrl

SCRIPT_DIR = Path(__file__).resolve().parent
ENV_FILE = SCRIPT_DIR / ".env"
LOG_FILE = SCRIPT_DIR / "dictation.log"

if IS_WINDOWS and BEEP:
    import winsound

try:
    import tkinter as tk
except Exception:
    tk = None
    SHOW_OVERLAY = False

# ----------------------------------------------------------------------------
# Logging (there is no console when launched with pythonw)
# ----------------------------------------------------------------------------


def setup_logging():
    if sys.stdout is not None:
        return
    try:
        if LOG_FILE.exists() and LOG_FILE.stat().st_size > 1_000_000:
            LOG_FILE.unlink()
        stream = open(LOG_FILE, "a", encoding="utf-8", buffering=1)
        sys.stdout = stream
        sys.stderr = stream
        print(f"\n--- started {time.strftime('%Y-%m-%d %H:%M:%S')} ---")
    except Exception:
        pass


def log(msg):
    try:
        print(msg)
    except Exception:
        pass


# ----------------------------------------------------------------------------
# Single instance
# ----------------------------------------------------------------------------


def claim_single_instance() -> bool:
    """False if another copy is already running."""
    if not IS_WINDOWS:
        return True
    try:
        ctypes.windll.kernel32.CreateMutexW(None, False, "DeepgramDictation_v1")
        return ctypes.windll.kernel32.GetLastError() != 183  # ALREADY_EXISTS
    except Exception:
        return True


# ----------------------------------------------------------------------------
# API key
# ----------------------------------------------------------------------------


def read_env_file() -> str:
    if not ENV_FILE.exists():
        return ""
    try:
        for line in ENV_FILE.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            name, value = line.split("=", 1)
            if name.strip() == "DEEPGRAM_API_KEY":
                return value.strip().strip('"').strip("'")
    except Exception as e:
        log(f"could not read .env: {e}")
    return ""


def get_api_key() -> str:
    key = os.environ.get("DEEPGRAM_API_KEY", "").strip() or read_env_file()
    if key:
        return key

    if sys.stdout is None:                      # windowless, cannot prompt
        alert("No API key found.\n\nRun setup.bat to enter your "
              "Deepgram API key.")
        sys.exit(1)

    print("No Deepgram API key found.")
    print("Get one at https://console.deepgram.com  ->  API Keys\n")
    key = input("Paste your API key and press Enter: ").strip()
    if not key:
        sys.exit("No key entered.")
    try:
        ENV_FILE.write_text(f"DEEPGRAM_API_KEY={key}\n", encoding="utf-8")
        print(f"Saved to {ENV_FILE}\n")
    except Exception as e:
        print(f"Could not save the key ({e}); using it for this session.\n")
    return key


def alert(msg: str):
    """Message box, for failures that happen with no console attached."""
    if IS_WINDOWS:
        try:
            ctypes.windll.user32.MessageBoxW(0, msg, "Deepgram Dictation", 0x10)
            return
        except Exception:
            pass
    log(msg)


# ----------------------------------------------------------------------------
# Shared state
# ----------------------------------------------------------------------------

ui = queue.Queue()          # worker thread -> overlay
mic_level = 0.0             # 0.0 .. 1.0, written by the audio callback


def beep(kind: str):
    if not (BEEP and IS_WINDOWS):
        return
    tones = {
        "start": [(880, 80)],
        "stop": [(660, 60), (440, 60)],
        "done": [(1046, 60)],
        "error": [(300, 180)],
    }
    for freq, ms in tones.get(kind, []):
        try:
            winsound.Beep(freq, ms)
        except Exception:
            pass


# ----------------------------------------------------------------------------
# Audio capture
# ----------------------------------------------------------------------------


class Recorder:
    def __init__(self):
        self._chunks = []
        self._stream = None
        self.active = False

    def _on_audio(self, indata, frames, time_info, status):
        global mic_level
        if status:
            log(f"audio: {status}")
        self._chunks.append(indata.copy())

        x = indata.astype(np.float32) / 32768.0
        rms = float(np.sqrt(np.mean(x * x))) + 1e-9
        db = 20.0 * math.log10(rms)
        # -60 dB reads as silence, -5 dB as full scale
        mic_level = max(0.0, min(1.0, (db + 60.0) / 55.0))

    def start(self):
        self._chunks = []
        self._stream = sd.InputStream(
            samplerate=SAMPLE_RATE,
            channels=CHANNELS,
            dtype="int16",
            callback=self._on_audio,
        )
        self._stream.start()
        self.active = True

    def stop(self):
        global mic_level
        self.active = False
        mic_level = 0.0
        if self._stream is not None:
            try:
                self._stream.stop()
                self._stream.close()
            except Exception:
                pass
            self._stream = None

        if not self._chunks:
            return None, 0.0

        pcm = np.concatenate(self._chunks, axis=0)
        seconds = len(pcm) / SAMPLE_RATE

        buf = io.BytesIO()
        with wave.open(buf, "wb") as wf:
            wf.setnchannels(CHANNELS)
            wf.setsampwidth(SAMPLE_WIDTH)
            wf.setframerate(SAMPLE_RATE)
            wf.writeframes(pcm.tobytes())
        return buf.getvalue(), seconds


# ----------------------------------------------------------------------------
# Floating meter
# ----------------------------------------------------------------------------

TRANSPARENT = "#010101"     # this exact colour is punched out of the window

PANEL_BG = "#17181d"
PANEL_EDGE = "#31333c"
TEXT_DIM = "#8b8f9c"
TEXT_BRIGHT = "#e7e9ee"
BAR_IDLE = (58, 61, 71)
BAR_IDLE_HEX = "#%02x%02x%02x" % BAR_IDLE
BAR_LOW = (56, 189, 160)
BAR_HIGH = (125, 211, 252)
REC_RED = "#f05454"
ERR_RED = "#ff6b6b"


class _POINT(ctypes.Structure):
    _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]


class _RECT(ctypes.Structure):
    _fields_ = [("left", ctypes.c_long), ("top", ctypes.c_long),
                ("right", ctypes.c_long), ("bottom", ctypes.c_long)]


class _MONITORINFO(ctypes.Structure):
    _fields_ = [("cbSize", ctypes.c_ulong), ("rcMonitor", _RECT),
                ("rcWork", _RECT), ("dwFlags", ctypes.c_ulong)]


def screen_work_area(win):
    """(left, top, right, bottom) of the usable area (screen minus the
    taskbar) of the monitor under the cursor. Falls back to the primary
    screen reported by tkinter."""
    if IS_WINDOWS:
        try:
            u = ctypes.windll.user32
            u.MonitorFromPoint.restype = ctypes.c_void_p
            u.MonitorFromPoint.argtypes = [_POINT, ctypes.c_ulong]
            u.GetMonitorInfoW.argtypes = [ctypes.c_void_p, ctypes.c_void_p]

            pt = _POINT()
            u.GetCursorPos(ctypes.byref(pt))
            MONITOR_DEFAULTTONEAREST = 2
            hmon = u.MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST)

            mi = _MONITORINFO()
            mi.cbSize = ctypes.sizeof(_MONITORINFO)
            if u.GetMonitorInfoW(hmon, ctypes.byref(mi)):
                r = mi.rcWork
                return r.left, r.top, r.right, r.bottom
        except Exception:
            pass
    return 0, 0, win.winfo_screenwidth(), win.winfo_screenheight()


def mix(c1, c2, t):
    t = max(0.0, min(1.0, t))
    return "#%02x%02x%02x" % tuple(
        int(a + (b - a) * t) for a, b in zip(c1, c2)
    )


def rounded(canvas, x1, y1, x2, y2, r, **kw):
    pts = [
        x1 + r, y1, x2 - r, y1, x2, y1, x2, y1 + r,
        x2, y2 - r, x2, y2, x2 - r, y2, x1 + r, y2,
        x1, y2, x1, y2 - r, x1, y1 + r, x1, y1,
    ]
    return canvas.create_polygon(pts, smooth=True, **kw)


class Overlay:
    W, H = 336, 68
    BARS = 38
    PITCH = 6
    BAR_W = 3
    MAX_BAR = 21

    def __init__(self, root):
        self.root = root
        self.state = "hidden"
        self.started_at = 0.0
        self.message = ""
        self.frame = 0
        self.history = deque([0.0] * self.BARS, maxlen=self.BARS)

        self.win = tk.Toplevel(root)
        self.win.withdraw()
        self.win.overrideredirect(True)
        self.win.attributes("-topmost", True)
        try:
            self.win.attributes("-transparentcolor", TRANSPARENT)
        except Exception:
            pass
        self.win.configure(bg=TRANSPARENT)

        self.c = tk.Canvas(self.win, width=self.W, height=self.H,
                           bg=TRANSPARENT, highlightthickness=0, bd=0)
        self.c.pack()

        rounded(self.c, 1, 1, self.W - 1, self.H - 1, 16,
                fill=PANEL_BG, outline=PANEL_EDGE, width=1)

        self.dot = self.c.create_oval(20, self.H // 2 - 5, 30, self.H // 2 + 5,
                                      fill=REC_RED, outline="")
        self.label = self.c.create_text(20, self.H // 2, anchor="w",
                                        text="", fill=TEXT_DIM,
                                        font=("Segoe UI", 10))
        self.timer = self.c.create_text(self.W - 18, self.H // 2, anchor="e",
                                        text="0:00", fill=TEXT_DIM,
                                        font=("Segoe UI", 10))

        cy = self.H // 2
        x0 = 42
        self.bar_x = [(x0 + i * self.PITCH, x0 + i * self.PITCH + self.BAR_W)
                      for i in range(self.BARS)]
        self.bars = [
            self.c.create_rectangle(x1, cy - 1, x2, cy + 1,
                                    fill=BAR_IDLE_HEX, outline="")
            for x1, x2 in self.bar_x
        ]

        self.place()
        self.win.update_idletasks()
        self.make_passthrough()

    # -- window plumbing --------------------------------------------------

    def place(self):
        left, top, right, bottom = screen_work_area(self.win)
        span = bottom - top
        x = left + (right - left - self.W) // 2
        if OVERLAY_POSITION == "top":
            y = top + int(span * 0.12)
        elif OVERLAY_POSITION == "taskbar":
            # sit just above the taskbar, using the desktop work area
            y = bottom - self.H - OVERLAY_MARGIN
        elif OVERLAY_POSITION == "bottom":
            y = top + int(span * 0.80)
        else:
            y = top + (span - self.H) // 2
        self.win.geometry(f"{self.W}x{self.H}+{x}+{y}")

    def make_passthrough(self):
        """Never take focus, never intercept a click."""
        if not IS_WINDOWS:
            return
        try:
            GWL_EXSTYLE = -20
            WS_EX_TRANSPARENT = 0x00000020
            WS_EX_TOOLWINDOW = 0x00000080
            WS_EX_NOACTIVATE = 0x08000000
            u = ctypes.windll.user32
            hwnd = u.GetParent(self.win.winfo_id()) or self.win.winfo_id()
            style = u.GetWindowLongW(hwnd, GWL_EXSTYLE)
            u.SetWindowLongW(hwnd, GWL_EXSTYLE, style | WS_EX_TRANSPARENT
                             | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE)
        except Exception as e:
            log(f"overlay passthrough unavailable: {e}")

    def show(self):
        self.place()
        self.win.deiconify()
        self.win.attributes("-topmost", True)

    def hide(self):
        self.win.withdraw()

    # -- states -----------------------------------------------------------

    def set_state(self, state, message=""):
        self.state = state
        self.message = message
        self.frame = 0

        if state == "recording":
            self.started_at = time.time()
            self.history = deque([0.0] * self.BARS, maxlen=self.BARS)
            self.c.itemconfigure(self.dot, state="normal")
            self.c.itemconfigure(self.label, state="hidden")
            self.c.itemconfigure(self.timer, state="normal", fill=TEXT_DIM)
            for b in self.bars:
                self.c.itemconfigure(b, state="normal")
            self.show()

        elif state == "working":
            self.c.itemconfigure(self.dot, state="hidden")
            self.c.itemconfigure(self.label, state="normal",
                                 text="Transcribing", fill=TEXT_BRIGHT)
            self.c.itemconfigure(self.timer, state="hidden")
            for b in self.bars:
                self.c.itemconfigure(b, state="normal")
            self.show()

        elif state == "error":
            self.c.itemconfigure(self.dot, state="hidden")
            self.c.itemconfigure(self.label, state="normal",
                                 text=message[:46], fill=ERR_RED)
            self.c.itemconfigure(self.timer, state="hidden")
            for b in self.bars:
                self.c.itemconfigure(b, state="hidden")
            self.started_at = time.time()
            self.show()

        else:
            self.hide()

    # -- per-frame --------------------------------------------------------

    def tick(self):
        self.frame += 1
        cy = self.H // 2

        if self.state == "recording":
            self.history.append(mic_level)
            for bar, (x1, x2), lvl in zip(self.bars, self.bar_x,
                                          self.history):
                h = max(1.5, lvl * self.MAX_BAR)
                self.c.coords(bar, x1, cy - h, x2, cy + h)
                colour = (BAR_IDLE_HEX if lvl < 0.02
                          else mix(BAR_LOW, BAR_HIGH, lvl))
                self.c.itemconfigure(bar, fill=colour)

            elapsed = int(time.time() - self.started_at)
            self.c.itemconfigure(
                self.timer, text=f"{elapsed // 60}:{elapsed % 60:02d}")

            # gentle pulse on the record dot
            p = 0.5 + 0.5 * math.sin(self.frame / 6.0)
            self.c.itemconfigure(self.dot, fill=mix((90, 40, 46),
                                                    (240, 84, 84), p))

        elif self.state == "working":
            head = (self.frame * 1.1) % (self.BARS + 10)
            for i, (bar, (x1, x2)) in enumerate(zip(self.bars, self.bar_x)):
                glow = max(0.0, 1.0 - abs(i - head) / 5.0)
                h = 1.5 + glow * 7
                self.c.coords(bar, x1, cy - h, x2, cy + h)
                self.c.itemconfigure(bar, fill=mix(BAR_IDLE, BAR_HIGH, glow))

        elif self.state == "error":
            if time.time() - self.started_at > 3.0:
                self.set_state("hidden")


# ----------------------------------------------------------------------------
# Deepgram
# ----------------------------------------------------------------------------


def transcribe(wav_bytes: bytes) -> str:
    resp = requests.post(
        ENDPOINT,
        params=DG_PARAMS,
        headers={"Authorization": f"Token {API_KEY}",
                 "Content-Type": "audio/wav"},
        data=wav_bytes,
        timeout=60,
    )
    resp.raise_for_status()
    alt = resp.json()["results"]["channels"][0]["alternatives"][0]
    text = alt.get("transcript", "").strip()
    return text.replace("<\\n\\n>", "\n\n").replace("<\\n>", "\n")


# ----------------------------------------------------------------------------
# Delivery
# ----------------------------------------------------------------------------

kbd = keyboard.Controller()


def deliver(text: str):
    previous = None
    if RESTORE_CLIPBOARD:
        try:
            previous = pyperclip.paste()
        except Exception:
            pass

    pyperclip.copy(text)
    if not AUTO_PASTE:
        return

    for mod in (keyboard.Key.alt, keyboard.Key.alt_l, keyboard.Key.alt_r,
                keyboard.Key.shift, keyboard.Key.cmd):
        try:
            kbd.release(mod)
        except Exception:
            pass
    time.sleep(0.12)

    with kbd.pressed(PASTE_MODIFIER):
        kbd.press("v")
        kbd.release("v")

    if previous is not None:
        time.sleep(CLIPBOARD_RESTORE_DELAY)
        try:
            # only restore if nothing else grabbed the clipboard meanwhile,
            # so we never clobber something the user copied after pasting
            if pyperclip.paste() == text:
                pyperclip.copy(previous)
        except Exception:
            pass


# ----------------------------------------------------------------------------
# Hotkeys
# ----------------------------------------------------------------------------

recorder = Recorder()
busy = threading.Lock()

MOD_KEYS = {
    "alt": {keyboard.Key.alt, keyboard.Key.alt_l,
            keyboard.Key.alt_r, keyboard.Key.alt_gr},
    "ctrl": {keyboard.Key.ctrl, keyboard.Key.ctrl_l, keyboard.Key.ctrl_r},
    "shift": {keyboard.Key.shift, keyboard.Key.shift_l, keyboard.Key.shift_r},
    "win": {keyboard.Key.cmd, keyboard.Key.cmd_l, keyboard.Key.cmd_r},
}
held = set()
fired = set()               # trigger keys currently held that already fired


def key_matches(key, wanted_char: str) -> bool:
    char = getattr(key, "char", None)
    if char and char.lower() == wanted_char:
        return True
    # with modifiers held the char can be a control code or symbol
    return getattr(key, "vk", None) == ord(wanted_char.upper())


def trigger_id(key):
    """Stable identity for a non-modifier key, across press and release."""
    return getattr(key, "vk", None) or getattr(key, "char", None)


def handle_toggle():
    if not busy.acquire(blocking=False):
        return
    try:
        if not recorder.active:
            try:
                recorder.start()
            except Exception as e:
                log(f"microphone unavailable: {e}")
                ui.put(("error", "Microphone unavailable"))
                beep("error")
                return
            ui.put(("state", "recording"))
            beep("start")
            return

        wav, seconds = recorder.stop()
        beep("stop")

        if wav is None or seconds < MIN_SECONDS:
            ui.put(("state", "hidden"))
            return

        ui.put(("state", "working"))
        try:
            text = transcribe(wav)
        except requests.HTTPError as e:
            code = e.response.status_code if e.response is not None else "?"
            body = e.response.text[:200] if e.response is not None else ""
            log(f"Deepgram {code}: {body}")
            ui.put(("error", "Key rejected - rerun setup.bat" if code == 401
                    else f"Deepgram error {code}"))
            beep("error")
            return
        except requests.RequestException as e:
            log(f"network: {e}")
            ui.put(("error", "No connection to Deepgram"))
            beep("error")
            return
        except Exception as e:
            log(f"{type(e).__name__}: {e}")
            ui.put(("error", type(e).__name__))
            beep("error")
            return

        if not text:
            ui.put(("error", "No speech detected"))
            return

        log(f"-> {text}")
        ui.put(("state", "hidden"))
        time.sleep(0.05)
        deliver(text)
        beep("done")
    finally:
        busy.release()


def on_press(key):
    for name, keys in MOD_KEYS.items():
        if key in keys:
            held.add(name)
            return

    kid = trigger_id(key)
    if kid in fired:            # keyboard auto-repeat while the key is held
        return

    # subset (not exact) match, so a stray/stuck extra modifier does not
    # silently disable the hotkey
    if HOTKEY_MODIFIERS <= held and key_matches(key, HOTKEY_CHAR):
        fired.add(kid)
        threading.Thread(target=handle_toggle, daemon=True).start()
    elif QUIT_MODIFIERS <= held and key_matches(key, QUIT_CHAR):
        fired.add(kid)
        ui.put(("quit", None))


def on_release(key):
    for name, keys in MOD_KEYS.items():
        if key in keys:
            held.discard(name)
            return
    fired.discard(trigger_id(key))


# ----------------------------------------------------------------------------

API_KEY = ""


def main():
    global API_KEY

    setup_logging()

    if not claim_single_instance():
        log("already running, exiting")
        sys.exit(0)

    if IS_WINDOWS:
        try:
            ctypes.windll.shcore.SetProcessDpiAwareness(1)
        except Exception:
            pass

    API_KEY = get_api_key()

    try:
        device = sd.query_devices(kind="input")
        log(f"microphone: {device['name']}")
    except Exception as e:
        alert(f"No microphone was found.\n\n{e}")
        sys.exit(1)

    combo = "+".join(sorted(HOTKEY_MODIFIERS) + [HOTKEY_CHAR]).upper()
    log(f"hotkey: {combo}   quit: CTRL+ALT+Q")

    listener = keyboard.Listener(on_press=on_press, on_release=on_release)
    listener.start()

    if not SHOW_OVERLAY or tk is None:
        try:
            listener.join()
        except KeyboardInterrupt:
            pass
        return

    root = tk.Tk()
    root.withdraw()
    overlay = Overlay(root)

    def pump():
        try:
            while True:
                kind, payload = ui.get_nowait()
                if kind == "quit":
                    root.quit()
                    return
                elif kind == "state":
                    overlay.set_state(payload)
                elif kind == "error":
                    overlay.set_state("error", payload)
        except queue.Empty:
            pass

        overlay.tick()
        root.after(33, pump)        # ~30 fps, also keeps Ctrl+C responsive

    root.after(33, pump)
    try:
        root.mainloop()
    except KeyboardInterrupt:
        pass

    listener.stop()
    if recorder.active:
        recorder.stop()
    log("stopped")


if __name__ == "__main__":
    main()
