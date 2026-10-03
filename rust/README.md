# Rust rewrite

This folder holds the app: `dictation.exe`, written in Rust. It replaced the Python version (`dictate.py`) in #12. For users, see the [main README](../README.md).

**Works now:**

- Global hotkeys: Alt+M (start/stop) and Ctrl+Alt+Q (quit).
- Deepgram streaming while you speak, with a batch fallback. Batch mode on request.
- The floating meter, with the live microphone level and the "Transcribing" animation.
- Paste at the cursor, and the text stays on the clipboard.
- Live paste (default): each phrase is pasted while you speak, only into the window that had focus at the start. In another window, the meter shows "Paused".
- Undo with **Ctrl+Alt+Z**: deletes the last dictation, if you have not typed since and the same window has focus.

- A setup window on the first start (or with `dictation.exe --setup`): it asks for the API key, checks it with Deepgram, saves it to `.env` next to the `.exe`, and can start the app at login.
- A log file, `dictation.log`, next to the `.exe`.
- Only one copy runs at a time (also not next to an old Python copy).

**Not done yet:** code signing (#19).

## Build

Install Rust from [rustup.rs](https://rustup.rs). Then, from this folder:

```bash
cargo build --release
```

The app is `target\release\dictation.exe`. A debug build (`cargo build`) shows a console with log messages.

**Smart App Control** blocks the Rust compiler and the unsigned `.exe`. Turn it off while you develop: Windows Security → App & browser control → Smart App Control settings.

The GNU toolchain has no `dlltool.exe`, so `native-tls` and `schannel` are pinned in `Cargo.toml` to versions that do not need it.

## Releases

CI builds and tests `dictation.exe` on every push and pull request (`.github/workflows/ci.yml`). A tag such as `v2.0.0` publishes it as a GitHub release (`.github/workflows/release.yml`).

The `.exe` is not code-signed yet (#19):

- SmartScreen shows **"Windows protected your PC"**. Select **More info → Run anyway**.
- With **Smart App Control** on, Windows blocks the app, and there is no way past it. The app can run on such a PC only after it is signed.

## Run

Quit any running copy first (Ctrl+Alt+Q). Only one copy runs at a time: the second one exits.

The app reads `DEEPGRAM_API_KEY` from the environment, or from the first `.env` file next to `dictation.exe` or in a folder above it. A build in `target\release` finds the `.env` in the repository root. If there is no key, the setup window opens. To change the key or the autostart later, run `dictation.exe --setup`.

1. Put the cursor in a text box.
2. Press **Alt+M** and speak.
3. Press **Alt+M** again. The text is pasted.
4. Press **Ctrl+Alt+Q** to quit.

## Environment variables

| Name | Effect |
|---|---|
| `DEEPGRAM_API_KEY` | The API key. |
| `DICTATION_MODE` | `batch` uploads the recording after you stop. Anything else, or not set: streaming. |
| `DICTATION_LIVE_PASTE` | `0` pastes all the text when you stop. Not set: live paste (streaming only). |
| `DICTATION_LOG` | Write the log to this file instead of `dictation.log` next to the `.exe`. |
| `DICTATION_FAKE_AUDIO` | A 16 kHz, mono, 16-bit WAV file that replaces the microphone (for the benchmark). |

## Differences from the old Python version

- The hotkeys use `RegisterHotKey`, so Alt+M no longer reaches the app that has focus. A small keyboard hook is still necessary for undo, but it only records *that* you typed, not which keys.
- Before each Ctrl+V, the app waits until no other program has the clipboard open. Windows clipboard history can block a paste otherwise (#18).
- After a live dictation, the full text goes to the clipboard 0.5 s after the last paste, so the target app pastes the last phrase, not the full text.
- The meter uses per-pixel alpha, so the rounded corners are smooth.
- The meter scales with the DPI of each monitor.
