"""Benchmark a version of the dictation app from the outside.

    python bench/bench.py
    python bench/bench.py --mode batch --live-paste off --exe path/to/dictation.exe

The script starts the app, presses the hotkeys with SendInput, and watches
the app's windows and a target text box. The app gets bench/sample.wav as
its microphone, so every run hears the same speech. Results go to
bench/results/ as JSON and Markdown.

Stop any running copy of the app first. Each trial sends about 10 seconds
of audio to Deepgram, so a run uses a little API credit.
"""

import argparse
import ctypes
import ctypes.wintypes as wt
import difflib
import json
import os
import platform
import queue
import re
import statistics
import subprocess
import sys
import threading
import time
import wave
from pathlib import Path

BENCH_DIR = Path(__file__).resolve().parent
ROOT = BENCH_DIR.parent
SAMPLE_WAV = BENCH_DIR / "sample.wav"
SAMPLE_TEXT = (BENCH_DIR / "sample.txt").read_text(encoding="utf-8").strip()
RESULTS_DIR = BENCH_DIR / "results"
APP_LOG = BENCH_DIR / "bench.log"        # the app logs here during a run
DEFAULT_EXE = ROOT / "rust" / "target" / "release" / "dictation.exe"

# the app's single-instance mutex, to detect a copy that is already running
APP_MUTEXES = ["DeepgramDictation_v1"]

READY_TIMEOUT = 30.0
PRESS_INTERVAL = 0.15       # between hotkey presses while waiting for startup
PASTE_TIMEOUT = 20.0
SETTLE = 1.5                # text box unchanged this long = all text is in

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
psapi = ctypes.WinDLL("psapi", use_last_error=True)

# ----------------------------------------------------------------------------
# Win32
# ----------------------------------------------------------------------------

VK_CONTROL, VK_LMENU = 0x11, 0xA4
KEYEVENTF_KEYUP = 0x0002
INPUT_KEYBOARD = 1


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [("wVk", wt.WORD), ("wScan", wt.WORD), ("dwFlags", wt.DWORD),
                ("time", wt.DWORD), ("dwExtraInfo", ctypes.c_size_t)]


class _INPUTUNION(ctypes.Union):
    # MOUSEINPUT is the largest member; pad to its size
    _fields_ = [("ki", KEYBDINPUT), ("_pad", ctypes.c_byte * 32)]


class INPUT(ctypes.Structure):
    _fields_ = [("type", wt.DWORD), ("u", _INPUTUNION)]


class PROCESS_MEMORY_COUNTERS_EX(ctypes.Structure):
    _fields_ = [("cb", wt.DWORD), ("PageFaultCount", wt.DWORD),
                ("PeakWorkingSetSize", ctypes.c_size_t),
                ("WorkingSetSize", ctypes.c_size_t),
                ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                ("PagefileUsage", ctypes.c_size_t),
                ("PeakPagefileUsage", ctypes.c_size_t),
                ("PrivateUsage", ctypes.c_size_t)]


def press(*vks):
    """Press the keys in order, then release them in reverse order."""
    downs = [(vk, 0) for vk in vks]
    ups = [(vk, KEYEVENTF_KEYUP) for vk in reversed(vks)]
    events = (INPUT * (len(vks) * 2))()
    for ev, (vk, flags) in zip(events, downs + ups):
        ev.type = INPUT_KEYBOARD
        ev.u.ki = KEYBDINPUT(vk, 0, flags, 0, 0)
    user32.SendInput(len(events), events, ctypes.sizeof(INPUT))


def press_toggle():
    press(VK_LMENU, ord("M"))


def press_quit():
    press(VK_CONTROL, VK_LMENU, ord("Q"))


WNDENUMPROC = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)


def visible_windows(pid):
    """Visible top-level windows of the process with a non-zero size."""
    found = []

    def cb(hwnd, _):
        owner = wt.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and user32.IsWindowVisible(hwnd):
            r = wt.RECT()
            user32.GetWindowRect(hwnd, ctypes.byref(r))
            if r.right > r.left and r.bottom > r.top:
                found.append(hwnd)
        return True

    user32.EnumWindows(WNDENUMPROC(cb), 0)
    return found


def meter_visible(pid):
    return bool(visible_windows(pid))


def wait_for(cond, timeout, step=0.001):
    """Poll cond() until it is true. Returns the time it became true, or None."""
    end = time.perf_counter() + timeout
    while time.perf_counter() < end:
        if cond():
            return time.perf_counter()
        time.sleep(step)
    return None


def any_app_running():
    SYNCHRONIZE = 0x00100000
    for name in APP_MUTEXES:
        h = kernel32.OpenMutexW(SYNCHRONIZE, False, name)
        if h:
            kernel32.CloseHandle(h)
            return name
    return None


class Process:
    """Reads CPU time and memory of a running process."""

    def __init__(self, pid):
        PROCESS_QUERY_INFORMATION, PROCESS_VM_READ = 0x0400, 0x0010
        self.handle = kernel32.OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, False, pid)

    def cpu_seconds(self):
        times = [wt.FILETIME() for _ in range(4)]
        kernel32.GetProcessTimes(self.handle, *[ctypes.byref(t) for t in times])

        def secs(ft):
            return ((ft.dwHighDateTime << 32) | ft.dwLowDateTime) / 1e7
        return secs(times[2]) + secs(times[3])   # kernel + user

    def memory(self):
        pmc = PROCESS_MEMORY_COUNTERS_EX()
        pmc.cb = ctypes.sizeof(pmc)
        psapi.GetProcessMemoryInfo(self.handle, ctypes.byref(pmc), pmc.cb)
        mb = 1024 * 1024
        return {"working_set_mb": pmc.WorkingSetSize / mb,
                "private_mb": pmc.PrivateUsage / mb,
                "peak_working_set_mb": pmc.PeakWorkingSetSize / mb}

    def close(self):
        kernel32.CloseHandle(self.handle)


# ----------------------------------------------------------------------------
# Target text box (runs on the main thread; the benchmark runs on a worker)
# ----------------------------------------------------------------------------


LRESULT = ctypes.c_ssize_t
WNDPROC = ctypes.WINFUNCTYPE(LRESULT, wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM)

WS_OVERLAPPEDWINDOW, WS_VISIBLE, WS_CHILD, WS_VSCROLL = 0x00CF0000, 0x10000000, 0x40000000, 0x00200000
WS_EX_TOPMOST = 0x00000008
ES_MULTILINE, ES_AUTOVSCROLL = 0x0004, 0x0040
WM_DESTROY, WM_CLOSE, WM_COMMAND, WM_APP = 0x0002, 0x0010, 0x0111, 0x8000
EN_CHANGE, EM_SETLIMITTEXT = 0x0300, 0x00C5
COLOR_WINDOW = 5


class WNDCLASSW(ctypes.Structure):
    _fields_ = [("style", wt.UINT), ("lpfnWndProc", WNDPROC),
                ("cbClsExtra", ctypes.c_int), ("cbWndExtra", ctypes.c_int),
                ("hInstance", wt.HINSTANCE), ("hIcon", wt.HICON),
                ("hCursor", wt.HANDLE), ("hbrBackground", wt.HBRUSH),
                ("lpszMenuName", wt.LPCWSTR), ("lpszClassName", wt.LPCWSTR)]


user32.DefWindowProcW.argtypes = [wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM]
user32.DefWindowProcW.restype = LRESULT
user32.CreateWindowExW.argtypes = [wt.DWORD, wt.LPCWSTR, wt.LPCWSTR, wt.DWORD,
                                   ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                                   wt.HWND, wt.HMENU, wt.HINSTANCE, wt.LPVOID]
user32.CreateWindowExW.restype = wt.HWND
user32.SendMessageW.argtypes = [wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM]
user32.SendMessageW.restype = LRESULT
user32.PostMessageW.argtypes = [wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM]
kernel32.GetModuleHandleW.restype = wt.HMODULE


class Target:
    """A standard Windows text box (EDIT control). A Tkinter text box
    dropped some pastes (#18), so the benchmark does not use Tkinter."""

    def __init__(self):
        self.changes = []           # perf_counter() of each change since clear()
        self.content = ""
        self._calls = queue.Queue()
        self._wndproc_ref = WNDPROC(self._wndproc)     # keep it alive

        instance = kernel32.GetModuleHandleW(None)
        wc = WNDCLASSW()
        wc.lpfnWndProc = self._wndproc_ref
        wc.hInstance = instance
        wc.hbrBackground = COLOR_WINDOW + 1
        wc.lpszClassName = "DictationBenchTarget"
        user32.RegisterClassW(ctypes.byref(wc))
        self.hwnd = user32.CreateWindowExW(
            WS_EX_TOPMOST, "DictationBenchTarget", "Benchmark paste target",
            WS_OVERLAPPEDWINDOW | WS_VISIBLE, 120, 120, 760, 220, None, None, instance, None)
        self.edit = user32.CreateWindowExW(
            0, "EDIT", "", WS_CHILD | WS_VISIBLE | WS_VSCROLL | ES_MULTILINE | ES_AUTOVSCROLL,
            0, 0, 740, 180, self.hwnd, None, instance, None)
        user32.SendMessageW(self.edit, EM_SETLIMITTEXT, 0, 0)    # no length limit

    def _text(self):
        n = user32.GetWindowTextLengthW(self.edit)
        buf = ctypes.create_unicode_buffer(n + 1)
        user32.GetWindowTextW(self.edit, buf, n + 1)
        return buf.value.replace("\r\n", "\n")

    def _wndproc(self, hwnd, msg, wp, lp):
        try:
            if msg == WM_COMMAND and (wp >> 16) == EN_CHANGE:
                self.changes.append(time.perf_counter())
                self.content = self._text()
                return 0
            if msg == WM_APP:
                while True:
                    try:
                        fn, done = self._calls.get_nowait()
                    except queue.Empty:
                        break
                    fn()
                    done.set()
                return 0
            if msg == WM_CLOSE:
                user32.DestroyWindow(hwnd)
                return 0
            if msg == WM_DESTROY:
                user32.PostQuitMessage(0)
                return 0
        except Exception as e:
            print(f"target window: {e}", file=sys.stderr)
        return user32.DefWindowProcW(hwnd, msg, wp, lp)

    def run(self):
        """The message loop. Returns when the window closes."""
        msg = wt.MSG()
        while user32.GetMessageW(ctypes.byref(msg), None, 0, 0) > 0:
            user32.TranslateMessage(ctypes.byref(msg))
            user32.DispatchMessageW(ctypes.byref(msg))

    def call(self, fn):
        """Run fn on the window's thread and wait for it."""
        done = threading.Event()
        self._calls.put((fn, done))
        user32.PostMessageW(self.hwnd, WM_APP, 0, 0)
        done.wait(5.0)

    def focus(self):
        def go():
            user32.SetForegroundWindow(self.hwnd)
            user32.SetFocus(self.edit)
        self.call(go)
        time.sleep(0.2)

    def clear(self):
        def go():
            user32.SetWindowTextW(self.edit, "")
            self.content = ""
            self.changes = []
        self.call(go)

    def close(self):
        self.call(lambda: user32.DestroyWindow(self.hwnd))


# ----------------------------------------------------------------------------
# Benchmark
# ----------------------------------------------------------------------------


def wav_seconds():
    with wave.open(str(SAMPLE_WAV), "rb") as w:
        return w.getnframes() / w.getframerate()


RUNNING = set()     # app processes started and not stopped yet


def launch(args):
    env = dict(os.environ,
               DICTATION_FAKE_AUDIO=str(SAMPLE_WAV),   # the app hears sample.wav
               DICTATION_LOG=str(APP_LOG),
               DICTATION_MODE=args.mode,
               DICTATION_LIVE_PASTE="1" if args.live_paste == "on" else "0")
    proc = subprocess.Popen([str(Path(args.exe).resolve())], cwd=str(ROOT), env=env)
    RUNNING.add(proc)
    return proc


def wait_until_ready(proc, target):
    """Press Alt+M until the meter appears. Returns seconds since launch."""
    started = time.perf_counter()
    while time.perf_counter() - started < READY_TIMEOUT:
        if proc.poll() is not None:
            raise RuntimeError(f"the app exited with code {proc.returncode}")
        pressed = time.perf_counter()
        press_toggle()
        if wait_for(lambda: meter_visible(proc.pid), PRESS_INTERVAL):
            # stop again at once; a recording under 0.4 s is ignored
            press_toggle()
            wait_for(lambda: not meter_visible(proc.pid), 5.0)
            return pressed - started
    raise RuntimeError("the hotkey never worked")


def run_trial(pid, target, transcribe, audio_seconds, log):
    target.clear()
    target.focus()
    result = {}

    t0 = time.perf_counter()
    press_toggle()
    shown = wait_for(lambda: meter_visible(pid), 5.0)
    result["hotkey_to_meter_ms"] = (shown - t0) * 1000 if shown else None

    # let the whole sample play, plus a short pause after the speech
    time.sleep(max(0.0, t0 + audio_seconds + 0.5 - time.perf_counter()))

    t2 = time.perf_counter()
    press_toggle()
    wait_for(lambda: not meter_visible(pid), PASTE_TIMEOUT)
    if transcribe:
        # with live paste, text arrives while speaking, so wait until the
        # text box has not changed for a while, then use the last change
        wait_for(lambda: (not target.changes and time.perf_counter() > t2 + PASTE_TIMEOUT)
                 or (target.changes and time.perf_counter() - target.changes[-1] > SETTLE),
                 PASTE_TIMEOUT + SETTLE)
        changes = list(target.changes)
        if changes:
            result["start_to_first_text_ms"] = (changes[0] - t0) * 1000
            result["stop_to_all_text_ms"] = max(0.0, changes[-1] - t2) * 1000
        result["text"] = target.content
        if log is not None:
            result["app_text"] = log.last_transcript()
    time.sleep(1.0)
    return result


class AppLog:
    """Reads the transcripts that the app writes to its log ("-> text")."""

    def __init__(self, path):
        self.path = Path(path)
        self.offset = self.path.stat().st_size if self.path.exists() else 0

    def last_transcript(self):
        if not self.path.exists():
            return None
        with open(self.path, encoding="utf-8", errors="replace") as f:
            f.seek(self.offset)
            new = f.read()
            self.offset = f.tell()
        lines = [ln[3:].strip() for ln in new.splitlines() if ln.startswith("-> ")]
        return lines[-1] if lines else None


def install_size(args):
    """Disk size in MB of what the app needs to run: only the .exe."""
    return {"app_mb": Path(args.exe).stat().st_size / (1024 * 1024), "runtime_mb": 0.0}


def words(text):
    """Lower-case words without punctuation, for the accuracy check."""
    return re.findall(r"[a-z0-9']+", text.lower())


def summarize(values):
    values = [v for v in values if v is not None]
    if not values:
        return None
    values.sort()
    p90 = values[min(len(values) - 1, round(0.9 * (len(values) - 1)))]
    return {"n": len(values), "min": values[0], "median": statistics.median(values),
            "p90": p90, "max": values[-1]}


def benchmark(args, target, out):
    audio_seconds = wav_seconds()
    startups = []
    for _ in range(args.startups - 1):      # extra launches, for the start time only
        target.focus()
        proc = launch(args)
        try:
            startups.append(wait_until_ready(proc, target))
        finally:
            stop(proc)
        time.sleep(1.0)

    target.focus()
    proc = launch(args)
    p = None
    try:
        startups.append(wait_until_ready(proc, target))
        out["startup_s"] = summarize(startups)
        p = Process(proc.pid)

        time.sleep(3.0)                 # let startup work settle
        cpu0, w0 = p.cpu_seconds(), time.perf_counter()
        time.sleep(args.idle_seconds)
        cpu1, w1 = p.cpu_seconds(), time.perf_counter()
        out["idle_cpu_percent"] = (cpu1 - cpu0) / (w1 - w0) * 100
        out["idle_memory"] = p.memory()

        log = AppLog(APP_LOG)
        trials = []
        for i in range(args.trials):
            r = run_trial(proc.pid, target, not args.no_transcribe, audio_seconds, log)
            if "text" in r:
                r["accuracy"] = difflib.SequenceMatcher(
                    None, words(SAMPLE_TEXT), words(r["text"])).ratio()
                r["exact_transcript"] = words(r["text"]) == words(SAMPLE_TEXT)
                if r.get("app_text") is not None:
                    # the text box has exactly what the app transcribed
                    r["paste_complete"] = words(r["text"]) == words(r["app_text"])
            trials.append(r)
            print(f"trial {i + 1}/{args.trials}: "
                  + ", ".join(f"{k}={v:.0f}" for k, v in r.items()
                              if k.endswith("_ms") and v is not None), flush=True)
        out["trials"] = trials
        out["after_trials_memory"] = p.memory()
        out["hotkey_to_meter_ms"] = summarize([t["hotkey_to_meter_ms"] for t in trials])
        if not args.no_transcribe:
            for key in ("start_to_first_text_ms", "stop_to_all_text_ms"):
                out[key] = summarize([t.get(key) for t in trials])
            out["accuracy"] = summarize([t.get("accuracy") for t in trials])
            out["exact_transcript_trials"] = sum(t.get("exact_transcript", False) for t in trials)
            if all("paste_complete" in t for t in trials):
                out["paste_complete_trials"] = sum(t["paste_complete"] for t in trials)
    finally:
        stop(proc)
        if p is not None:
            p.close()


def stop(proc):
    press_quit()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()
    RUNNING.discard(proc)


def app_settings(args):
    """The settings that change the results."""
    return f"mode={args.mode}, live paste={args.live_paste if args.mode == 'streaming' else 'off'}"


def git_commit():
    try:
        return subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT,
                              capture_output=True, text=True).stdout.strip()
    except Exception:
        return ""


def to_markdown(out):
    def ms(s):
        return (f"{s['median']:.0f} ms (min {s['min']:.0f}, p90 {s['p90']:.0f}, "
                f"max {s['max']:.0f}, n={s['n']})") if s else "n/a"

    def count(key):
        return f"{out[key]} of {len(out['trials'])}" if key in out else "n/a"

    mem = out["idle_memory"]
    size = out["install"]
    rows = [
        ("Start time (launch until Alt+M works)",
         f"{out['startup_s']['median']:.2f} s (min {out['startup_s']['min']:.2f}, "
         f"max {out['startup_s']['max']:.2f}, n={out['startup_s']['n']})"),
        ("Idle memory (working set)", f"{mem['working_set_mb']:.1f} MB"),
        ("Idle memory (private)", f"{mem['private_mb']:.1f} MB"),
        ("Peak memory (working set, after trials)",
         f"{out['after_trials_memory']['peak_working_set_mb']:.1f} MB"),
        ("Idle CPU", f"{out['idle_cpu_percent']:.2f} %"),
        ("Hotkey to meter", ms(out["hotkey_to_meter_ms"])),
        ("Start to first text", ms(out.get("start_to_first_text_ms"))),
        ("Stop to all text in place", ms(out.get("stop_to_all_text_ms"))),
        ("Paste complete (text box = the app's transcript)", count("paste_complete_trials")),
        ("Exact transcript (text box = sample.txt)", count("exact_transcript_trials")),
        ("Transcript accuracy (word match, median)",
         f"{out['accuracy']['median'] * 100:.0f} %" if out.get("accuracy") else "n/a"),
        ("Disk (app and runtime)", f"{size['app_mb'] + size['runtime_mb']:.1f} MB"),
    ]
    lines = [f"# Benchmark: {out['label']}", "",
             f"- Date: {out['date']}",
             f"- App: {out['app']}, commit `{out['commit']}`",
             f"- Machine: {out['machine']}",
             f"- Trials: {out['trials_requested']}, idle sample: {out['idle_seconds']} s",
             f"- Settings: {out['settings']}",
             "- Start time has a resolution of about 150 ms (one hotkey press per 150 ms).",
             "", "| Metric | Result |", "|---|---|"]
    lines += [f"| {k} | {v} |" for k, v in rows]
    if out.get("trials") and "text" in out["trials"][0]:
        lines += ["", "First transcript:", "", f"> {out['trials'][0]['text']}"]
    return "\n".join(lines) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--exe", default=str(DEFAULT_EXE), help="path to dictation.exe")
    ap.add_argument("--mode", choices=["streaming", "batch"], default="streaming")
    ap.add_argument("--live-paste", choices=["on", "off"], default="on",
                    help="streaming only: paste each phrase while speaking")
    ap.add_argument("--trials", type=int, default=10)
    ap.add_argument("--idle-seconds", type=float, default=10.0)
    ap.add_argument("--startups", type=int, default=3,
                    help="how many launches to time for the start time")
    ap.add_argument("--no-transcribe", action="store_true",
                    help="skip the stop-to-paste measurement (for the prototype)")
    ap.add_argument("--label", help="name for the results files")
    args = ap.parse_args()
    if not Path(args.exe).exists():
        ap.error(f"{args.exe} not found; build it with 'cargo build --release' in rust/")

    running = any_app_running()
    if running:
        sys.exit(f"A copy of the app is running (mutex {running}). Stop it first.")

    user32.SetProcessDPIAware()
    label = args.label or f"rust-{args.mode}-{time.strftime('%Y-%m-%d')}"
    out = {"label": label, "app": "rust", "date": time.strftime("%Y-%m-%d %H:%M"),
           "commit": git_commit(), "trials_requested": args.trials,
           "idle_seconds": args.idle_seconds,
           "machine": f"{platform.platform()}, {os.cpu_count()} logical CPUs",
           "install": install_size(args), "settings": app_settings(args)}

    target = Target()
    errors = []

    def work():
        try:
            benchmark(args, target, out)
        except Exception as e:
            errors.append(e)
        finally:
            target.close()

    worker = threading.Thread(target=work, daemon=True)
    worker.start()
    target.run()
    if worker.is_alive():
        # the target window was closed during the run (#21): the worker's
        # cleanup will not run, so stop the app here and save nothing
        for proc in list(RUNNING):
            stop(proc)
        sys.exit("benchmark interrupted: the target window was closed; no results saved")
    if errors:
        sys.exit(f"benchmark failed: {errors[0]}")

    RESULTS_DIR.mkdir(exist_ok=True)
    (RESULTS_DIR / f"{label}.json").write_text(json.dumps(out, indent=2), encoding="utf-8")
    md = to_markdown(out)
    (RESULTS_DIR / f"{label}.md").write_text(md, encoding="utf-8")
    print("\n" + md)


if __name__ == "__main__":
    main()
