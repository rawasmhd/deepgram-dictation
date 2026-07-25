# Security Policy

## Supported versions

This is a small, single-file tool developed on a rolling basis. Only the latest
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

- Your Deepgram API key is stored locally in `.env`, which is git-ignored and
  never committed. Treat it like a password; if it is exposed, revoke it in the
  [Deepgram console](https://console.deepgram.com).
- Recorded audio is sent to Deepgram over HTTPS for transcription and is not
  stored by this tool. See Deepgram's own policies for how they handle it.
- The app registers a global keyboard hook (to detect the hotkey) and simulates
  keystrokes (to paste). This is required for its function; the full source is
  in `dictate.py` for review.
