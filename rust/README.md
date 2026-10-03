# Rust rewrite

This folder holds the Rust version of `dictate.py` (tracking issue #12). It is not complete yet.

**Works now:**

- Global hotkeys: Alt+M (start/stop) and Ctrl+Alt+Q (quit).
- Deepgram streaming while you speak, with a batch fallback. Batch mode on request.
- The floating meter, with the live microphone level and the "Transcribing" animation.
- Paste at the cursor, and the text stays on the clipboard.

**Not ported yet:** live paste and undo (#9), setup scripts and logging (#10), signed releases (#11).

## Build

Install Rust from [rustup.rs](https://rustup.rs). Then, from this folder:

```bash
cargo build --release
```

The app is `target\release\dictation.exe`. A debug build (`cargo build`) shows a console with log messages.

**Smart App Control** blocks the Rust compiler and the unsigned `.exe`. Turn it off while you develop: Windows Security → App & browser control → Smart App Control settings.

The GNU toolchain has no `dlltool.exe`, so `native-tls` and `schannel` are pinned in `Cargo.toml` to versions that do not need it.

## Run

Stop the Python version first (`scripts\Stop Dictation.bat`). The Rust version does not start while the Python version runs.

The app reads `DEEPGRAM_API_KEY` from the environment, or from the first `.env` file next to `dictation.exe` or in a folder above it. A build in `target\release` finds the `.env` in the repository root.

1. Put the cursor in a text box.
2. Press **Alt+M** and speak.
3. Press **Alt+M** again. The text is pasted.
4. Press **Ctrl+Alt+Q** to quit.

## Environment variables

| Name | Effect |
|---|---|
| `DEEPGRAM_API_KEY` | The API key. |
| `DICTATION_MODE` | `batch` uploads the recording after you stop. Anything else, or not set: streaming. |
| `DICTATION_LOG` | A file that gets a copy of the log, with one `-> text` line per dictation. |
| `DICTATION_FAKE_AUDIO` | A 16 kHz, mono, 16-bit WAV file that replaces the microphone (for the benchmark). |

## Differences from dictate.py

- The hotkeys use `RegisterHotKey`, not a global keyboard hook. Alt+M no longer reaches the app that has focus. Antivirus tools are less likely to flag it.
- The meter uses per-pixel alpha, so the rounded corners are smooth.
- The meter scales with the DPI of each monitor.
