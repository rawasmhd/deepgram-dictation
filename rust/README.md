# Rust rewrite (prototype)

This folder holds the Rust version of `dictate.py`. It is a prototype. It tests the hard parts of the rewrite:

- the global hotkey (Alt+M, Ctrl+Alt+Q)
- the paste at the cursor
- the floating meter, with the live microphone level

It does not call Deepgram yet. When you stop recording, it pastes a test sentence.

## Build

Install Rust from [rustup.rs](https://rustup.rs). Then, from this folder:

```bash
cargo build --release
```

The app is `target\release\dictation.exe`. A debug build (`cargo build`) shows a console with log messages.

**Smart App Control** blocks the Rust compiler and the unsigned `.exe`. Turn it off while you develop: Windows Security → App & browser control → Smart App Control settings.

## Run

Stop the Python version first (`scripts\Stop Dictation.bat`). The prototype does not start while the Python version runs.

1. Put the cursor in a text box.
2. Press **Alt+M** and speak. The meter shows the level.
3. Press **Alt+M** again. The test sentence is pasted.
4. Press **Ctrl+Alt+Q** to quit.

## Differences from dictate.py

- The hotkeys use `RegisterHotKey`, not a global keyboard hook. Alt+M no longer reaches the app that has focus. Antivirus tools are less likely to flag it.
- The meter uses per-pixel alpha, so the rounded corners are smooth.
- The meter scales with the DPI of each monitor.
