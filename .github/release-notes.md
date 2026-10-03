## Install

1. Download `dictation.exe`.
2. Put it in a folder of your choice, with a `.env` file next to it that contains your Deepgram API key:
   ```
   DEEPGRAM_API_KEY=your-key
   ```
3. Double-click `dictation.exe`. Then press **Alt+M** anywhere to dictate.

## Windows security prompts

`dictation.exe` is not code-signed yet (#19). So Windows may warn you the first time:

- **"Windows protected your PC"** (SmartScreen): select **More info → Run anyway**.
- **"Smart App Control blocked an app"**: Windows does not let you run unsigned apps while Smart App Control is on. Use the Python version (`dictate.py`) until the `.exe` is signed.
- **Antivirus warnings**: the app registers global hotkeys, pastes with a simulated Ctrl+V, and uses a keyboard hook to notice typing for undo. The source code is in this repository.
