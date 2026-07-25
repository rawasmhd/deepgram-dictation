# Contributing

Thanks for your interest — contributions are welcome, whether it's a bug fix, a
new feature, or a whole new platform.

## Getting set up

```bash
git clone https://github.com/rawasmhd/deepgram-dictation
cd deepgram-dictation
python -m pip install sounddevice numpy requests pynput pyperclip
python dictate.py            # runs with a visible console for debugging
```

The whole app is one file, `dictate.py`. The tunable behaviour lives in the
constants at the top (hotkey, paste mode, overlay position, Deepgram params) —
start there to get your bearings.

## Submitting changes

1. Fork the repo and create a branch off `main`.
2. Keep changes focused; match the surrounding style (this codebase favours
   small functions, plain constants, and cross-platform guards over clever
   abstractions).
3. Open a pull request describing what you changed and how you tested it.

Please don't commit secrets — `.env` is git-ignored for a reason.

## Help wanted: macOS support 🍎

This is the big one, and a great first contribution if you're on a Mac. The
core stack (`sounddevice`, `requests`, `pynput`, `pyperclip`, `tkinter`) is all
cross-platform, and the code already has some scaffolding for it —
`IS_MAC` is detected and `PASTE_MODIFIER` already selects **Cmd** instead of
Ctrl on macOS. But several Windows-specific pieces need a macOS path before it
works end to end:

- **Hotkey detection.** `key_matches()` falls back to `vk == ord("M")`, which is
  a *Windows* virtual-key code. On macOS the keycode differs and holding Option
  turns `key.char` from `"m"` into `"µ"`, so **Alt+M never fires**. This needs
  Mac-specific keycode handling (and possibly a different default hotkey, since
  Option+letter types special characters).

- **Permissions.** `pynput`'s global listener and its keystroke simulation both
  require **Accessibility** + **Input Monitoring** grants in System Settings →
  Privacy & Security, and the mic needs **Microphone** permission. Without them
  the listener and auto-paste silently do nothing. At minimum this needs
  documenting; ideally a first-run check that points the user to the right pane.

- **The floating overlay.** `-transparentcolor` (used in `Overlay.__init__`) is
  Windows/X11 only — unsupported on macOS Aqua — so the panel renders as a dark
  square instead of a floating rounded meter. And `make_passthrough()` is
  Windows-only, so on macOS the overlay may steal focus (which breaks
  paste-at-cursor). macOS needs a different transparency approach
  (`-transparent` / `-alpha`) and a way to keep the window non-activating.

- **Setup & launch scripts.** The `.bat` files, `pythonw.exe`, the single-
  instance mutex, and the Startup-folder autostart are all Windows-only. The
  macOS equivalent would be a `setup.command` / start / stop shell script set,
  running detached, and a `launchd` plist or Login Item for autostart.

None of these change how the app works on Windows — please keep the Windows
path intact and gate macOS behaviour behind `IS_MAC` (and Linux behind the
same pattern if you're feeling ambitious). If you're picking this up, opening
an issue first to coordinate is appreciated.

## Reporting bugs

Open an issue with your OS version, what you expected, what happened, and
anything from `dictation.log` (next to `dictate.py`). Running
`Troubleshoot.bat` shows errors in a visible console.
