# deepgram-dictation

Minimal push-to-talk dictation for Windows. Press **Alt+M**, speak, press **Alt+M** again, and the transcribed text is pasted at your cursor — in any app. The only thing on screen is a small level meter that floats above the taskbar while you talk.

Audio goes to [Deepgram](https://deepgram.com) for transcription; nothing is stored.

## Features

- **Alt+M** to start, **Alt+M** to stop, transcribe, and paste
- **Ctrl+Alt+Q** to quit
- Works in any application — the text is pasted wherever your cursor is
- Floating microphone meter while recording; a "Transcribing" animation while it works
- Runs silently in the background, no console window, no taskbar clutter
- Restores whatever was on your clipboard afterwards
- Smart formatting and spoken punctuation ("comma", "new paragraph") via Deepgram's `nova-3`

## Install

Requires **Python 3** on your PATH ([python.org](https://www.python.org/downloads/) — tick *Add python.exe to PATH*) and a Deepgram API key ([console.deepgram.com](https://console.deepgram.com) → API Keys; new accounts get free credit).

Double-click **`setup.bat`**. It will:

1. install the Python packages (`sounddevice`, `numpy`, `requests`, `pynput`, `pyperclip`)
2. ask for your Deepgram API key and save it to `.env`
3. register itself to start at login
4. launch it

That's it. Press **Alt+M** anywhere to dictate.

## Windows security prompts

This is an unsigned script (not a code-signed `.exe`), so Windows may warn you the first time you run `setup.bat`. There's nothing malicious here — the full source is right in front of you — but here's how to get past the friction:

- **Cleanest fix: clone instead of downloading the ZIP.** Files pulled by `git clone` aren't tagged with the "Mark of the Web", so no warning appears at all:
  ```bash
  git clone https://github.com/rawasmhd/deepgram-dictation
  ```
- **If you downloaded the ZIP** and see *"Windows protected your PC"*, click **More info → Run anyway**. Or unblock the files first, from a PowerShell window in the folder:
  ```bash
  Get-ChildItem *.bat | Unblock-File
  ```
- **Antivirus may flag it.** To detect Alt+M the app installs a global keyboard hook, and it simulates Ctrl+V to paste — behaviour that looks keylogger-like to some scanners. That's inherent to how a background dictation hotkey works; read `dictate.py` if you want to confirm what it does.

## Usage

1. Put your cursor where you want the text.
2. Press **Alt+M** — the meter appears above your taskbar.
3. Speak.
4. Press **Alt+M** again — the meter shows "Transcribing", then the text is pasted.

Taps shorter than 0.4s are ignored, so a stray press won't send an empty request.

The other batch files:

- **`Start Dictation.bat`** — relaunch after you've quit with Ctrl+Alt+Q
- **`Stop Dictation.bat`** — stop the background process
- **`Troubleshoot.bat`** — run with a visible console to see errors

## How it works

A single `dictate.py` runs in the background under `pythonw.exe` (so there's no console window). A global keyboard listener ([pynput](https://pypi.org/project/pynput/)) watches for the hotkey.

On the first Alt+M it opens a 16 kHz mono microphone stream with [sounddevice](https://pypi.org/project/sounddevice/) and buffers the audio. On the second Alt+M it packs the buffer into an in-memory WAV, POSTs it to Deepgram's `/v1/listen`, pulls the transcript out of the response, copies it to the clipboard, and simulates **Ctrl+V** to paste it — then restores your previous clipboard contents.

The floating meter is a borderless, click-through Tkinter overlay. On Windows it's given the `WS_EX_TRANSPARENT | WS_EX_NOACTIVATE` styles so it never steals focus or intercepts a click.

## Customizing

The knobs are constants at the top of `dictate.py`:

| Constant | Default | What it does |
|---|---|---|
| `HOTKEY_MODIFIERS` / `HOTKEY_CHAR` | `{"alt"}` / `"m"` | The start/stop hotkey |
| `AUTO_PASTE` | `True` | `False` = copy to clipboard only, don't paste |
| `RESTORE_CLIPBOARD` | `True` | Put your previous clipboard back after pasting |
| `BEEP` | `False` | Short tones on start / stop / done / error |
| `MIN_SECONDS` | `0.4` | Ignore taps shorter than this |
| `OVERLAY_POSITION` | `"taskbar"` | `center` \| `top` \| `bottom` \| `taskbar` |
| `OVERLAY_MARGIN` | `12` | Gap above the taskbar, in pixels |
| `DG_PARAMS` | `nova-3`, `en` | Deepgram model, language, formatting options |

To dictate in another language, change `"language"` in `DG_PARAMS` (see [Deepgram's language list](https://developers.deepgram.com/docs/models-languages-overview)).

## Troubleshooting

**Nothing happens on Alt+M.** Make sure it's running (`Start Dictation.bat`). Run `Troubleshoot.bat` to see errors in a console.

**"Key rejected".** The API key is wrong or expired — rerun `setup.bat` and paste a fresh one.

**"No connection to Deepgram".** Check your internet connection.

**"Microphone unavailable".** Another app may be holding the mic, or Windows mic permissions are off (Settings → Privacy → Microphone).

**It pastes into the wrong place.** The text is pasted wherever focus is when transcription finishes — click into the target field before pressing Alt+M the second time.

## License

MIT — see [LICENSE](LICENSE).
