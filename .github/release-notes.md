## Install

1. Download `dictation.exe`.
2. Put it in a folder that you keep, for example `%LOCALAPPDATA%\DeepgramDictation`.
3. Double-click it. A setup window asks for your Deepgram API key (from [console.deepgram.com](https://console.deepgram.com)) and can start the app at login.
4. Press **Alt+M** anywhere to dictate.

The app shows a microphone icon in the notification area (select **^** on the taskbar if you do not see it). Right-click it for the settings, start at login, the log file and quit.

Upgrading from the Python version: see "Upgrading from the Python version" in the [README](https://github.com/rawasmhd/deepgram-dictation#upgrading-from-the-python-version).

## Windows security prompts

`dictation.exe` is not code-signed yet. Code signing is being set up with [SignPath Foundation](https://signpath.org), which provides free code signing for open-source projects (#19). Until then, Windows may warn you the first time:

- **"Windows protected your PC"** (SmartScreen): select **More info → Run anyway**.
- **"Smart App Control blocked an app"**: Windows does not let you run unsigned apps while Smart App Control is on. The app can run on such a PC only after it is signed.
- **Antivirus warnings**: the app registers global hotkeys, pastes with a simulated Ctrl+V, and uses a keyboard hook to notice typing for undo. The source code is in this repository.

## Privacy and code signing

- [Privacy policy](https://github.com/rawasmhd/deepgram-dictation/blob/main/PRIVACY.md)
- [Code signing policy](https://github.com/rawasmhd/deepgram-dictation/blob/main/CODE_SIGNING.md)
