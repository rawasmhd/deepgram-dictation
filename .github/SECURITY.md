# Security Policy

## Supported versions

This is a small tool developed on a rolling basis. Only the latest
commit on `main` and the most recent release are supported.

| Version | Supported |
|---|---|
| latest `main` / newest release | ✅ |
| older | ❌ |

## Reporting a vulnerability

Please report security issues **privately**, not in a public issue.

- Email: rawas@applab.qa
- Or use GitHub's private reporting: the repository's **Security** tab →
  **Report a vulnerability**.

Please include steps to reproduce and the affected commit or release. I aim to
acknowledge reports within a few days.

## Handling of secrets and data

- Your Deepgram API key is stored locally in `.env` next to `dictation.exe`,
  which is git-ignored and never committed. Treat it like a password; if it is exposed, revoke it in the
  [Deepgram console](https://console.deepgram.com).
- Recorded audio is sent to Deepgram over HTTPS for transcription and is not
  stored by this tool. See Deepgram's own policies for how they handle it.
- The app registers global hotkeys, simulates keystrokes (to paste and undo),
  and uses a keyboard hook that only notices *that* a key was pressed (to
  cancel undo). This is required for its function; the full source is in
  `rust/src/` for review.
- `dictation.log` next to `dictation.exe` contains the transcribed text. Delete
  it whenever you like.
