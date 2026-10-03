# Python vs Rust, 2026-10-03

Measured with `bench/bench.py` on the same PC (Windows 11 25H2, 16 logical CPUs). Python: `dictate.py` on `main`. Rust: `dictation.exe` from `feature/rust-rewrite` (#12), release build.

## Summary

The Rust version is **as fast as the Python version where the network decides**, and much lighter everywhere else.

| Metric | Python | Rust | Change |
|---|---|---|---|
| Start time (launch until Alt+M works) | 0.77–0.93 s | 0.15 s | about 6x faster |
| Idle memory (working set) | 62.9 MB | 14.3 MB | 77 % less |
| Idle memory (private) | 517 MB | 3.4 MB | 99 % less |
| Peak memory (working set) | 67.6 MB | 16.5 MB | 76 % less |
| Idle CPU | 0.62 % | 0.00 % | |
| Hotkey to meter (median) | 31 ms | 7 ms | about 4x faster |
| Disk (app + runtime) | 200 MB | 1.0 MB | 99.5 % less |
| Install | Python, 6 packages, `setup.bat` | One `.exe`, setup window on the first start | |

## Transcription (live paste, the default mode)

Both runs today, one after the other, 10 trials each.

| Metric | Python | Rust |
|---|---|---|
| Start to first text (median) | 3.02 s | 3.35 s |
| Start to first text (p90) | 3.26 s | 5.01 s |
| **Stop to all text in place (median)** | **0 ms** | **0 ms** |
| Stop to all text in place (p90) | 644 ms | 0 ms |
| Paste complete | 10 of 10 | 9 of 10 |
| Exact transcript | 10 of 10 | 9 of 10 |

The first text depends on when Deepgram finishes the first phrase. The differences between the two apps here are network noise: the Rust p90 comes from 2 slow trials (5.0 s and 8.8 s), which also happen in Python runs on other days.

## Transcription (streaming and batch)

These modes were not compared on the same day. The full run was stopped during the Python streaming run, and the network was unstable that day (Python streaming varied from 0.47 s to 6.1 s). Both versions send the same audio to the same Deepgram service, so the app cannot change these times much.

| Metric | Python (2026-10-02, 10 trials) | Rust (2026-10-03) |
|---|---|---|
| Streaming: stop to all text (median) | 523 ms | 528 ms (3 trials, 431–528 ms) |
| Batch: stop to all text (median) | 3250 ms | not measured (1 manual test, correct text) |

## Lost text during the paste (#18)

| Version | Live paste: paste complete |
|---|---|
| Python | 9 of 10 (2026-10-02), 10 of 10 (today): **19 of 20** |
| Rust | 10 of 10 (2026-10-03, earlier run), 9 of 10 (today): **19 of 20** |

The Rust version waits until no other program has the clipboard open before each Ctrl+V (#18 workaround). These numbers do not show a clear improvement. In the lost Rust trial, the last 2 phrases were logged by the app but did not arrive in the text box. This is the same pattern as in the Python version.

## Files

- Today: `python-live-paste-2026-10-03.*`, `rust-live-paste-2026-10-03.*`.
- Python baseline: `python-baseline-*.*` (2026-10-02). See `README.md`.
- The Rust streaming numbers come from a 3-trial test run on 2026-10-03. That result file was not kept.
