# Privacy policy

deepgram-dictation runs on your PC. It has no server of its own and collects no usage data.

## What leaves your PC

- **Audio, only while you record.** Between the first and the second press of Alt+M, the app sends the microphone audio to [Deepgram](https://deepgram.com) for transcription, over an encrypted connection (TLS), with your own Deepgram API key. Deepgram's [privacy policy](https://deepgram.com/privacy) applies to that audio.
- **Nothing else.** The app sends no telemetry, no crash reports and no other data.

## What stays on your PC

- **Your API key**, in the `.env` file next to the app.
- **The transcribed text**, on the clipboard. If Windows clipboard history is on, Windows keeps a copy there too.
- **A log file** (`dictation.log`), which contains the transcribed text of each dictation. You can delete it at any time. The app deletes it when it is larger than 1 MB.

## Keyboard

The app registers global hotkeys (Alt+M, Ctrl+Alt+Z, Ctrl+Alt+Q) and simulates Ctrl+V and Backspace to paste and undo. It also watches the keyboard, only to detect the hotkeys and to notice that you typed (which cancels undo). It stores and sends no keystrokes.

## Contact

Questions: open an issue at https://github.com/rawasmhd/deepgram-dictation/issues.
