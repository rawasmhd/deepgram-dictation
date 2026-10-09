# Contributing

Thanks for your interest. Contributions are welcome, whether it's a bug fix, a new feature, or a whole new platform.

## Getting set up

Install [Rust](https://rustup.rs). Then:

```bash
git clone https://github.com/rawasmhd/deepgram-dictation
cd deepgram-dictation/rust
cargo build          # debug build: runs with a console that shows the log
cargo test
```

Put your Deepgram key in a `.env` file in the repository root (`DEEPGRAM_API_KEY=...`). A build in `rust/target/` finds it there. See [rust/README.md](../rust/README.md) for the environment variables and the toolchain notes.

The code is in `rust/src/`:

| File | What it does |
|---|---|
| `main.rs` | Start, hotkeys, the record/stop flow, undo |
| `audio.rs` | Microphone capture, converted to 16 kHz mono |
| `deepgram.rs` | Streaming (WebSocket) and batch transcription |
| `live.rs` | Live paste: what to paste, and when |
| `paste.rs` | Clipboard and simulated keys |
| `overlay.rs` | The floating meter |
| `setup.rs`, `config.rs` | First-run setup window, `.env`, autostart |
| `typing.rs` | Notices typing, to cancel undo |
| `logging.rs` | `dictation.log` |

## Submitting changes

1. Open an issue first, or find an existing one. Every change needs an issue before work starts.
2. Fork the repo and create a branch off `main`.
3. Keep changes focused, and match the surrounding style: small functions, plain constants, and a short comment where the reason is not obvious.
4. Run `cargo build` (no warnings) and `cargo test`.
5. Open a pull request that says what you changed and how you tested it, and link the issue (for example `Closes #8`).

If a change can affect speed or reliability, run the benchmark in [`bench/`](../bench) before and after.

If several people or agents work at the same time, follow the rules in [CLAUDE.md](../CLAUDE.md) ("several agents at the same time") and open issues with the task template.

Please don't commit secrets. `.env` is git-ignored for a reason.

## Help wanted: macOS support

This is the big one ([#1](https://github.com/rawasmhd/deepgram-dictation/issues/1)). The audio capture (`cpal`) and the Deepgram code (`tungstenite`, `ureq`) are cross-platform already. These parts call Windows directly and need a macOS version:

- **Hotkeys** (`main.rs`): `RegisterHotKey`. macOS needs a global hotkey API (for example Carbon `RegisterEventHotKey`). Option+letter types special characters on a Mac, so the default hotkey may need to change.
- **Paste and undo** (`paste.rs`): the clipboard and `SendInput`. macOS needs `NSPasteboard` and `CGEvent` with **Cmd**+V. Both need the **Accessibility** permission, and the app should point the user to it on the first start.
- **Typing detection** (`typing.rs`): a low-level keyboard hook. macOS needs an event tap, which needs **Input Monitoring** permission.
- **The meter** (`overlay.rs`): a layered Windows window drawn with GDI. macOS needs a borderless, non-activating, click-through `NSPanel`.
- **Setup, autostart, single instance** (`setup.rs`, `config.rs`, `main.rs`): a Win32 window, the Run registry key and a named mutex. macOS needs a small window, a Login Item, and a lock file.

Please keep the Windows code as it is, and put macOS code behind `#[cfg(target_os = "macos")]`. If you pick this up, comment on #1 first to coordinate.

## Reporting bugs

Open an issue with your Windows version, what you expected, what happened, and the lines from `dictation.log` (next to `dictation.exe`) around the problem.
