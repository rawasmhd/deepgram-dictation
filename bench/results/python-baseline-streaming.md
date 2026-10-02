# Benchmark: python-baseline-streaming

- Date: 2026-10-01 16:42
- App: python, commit `7c40ed3`
- Machine: Windows-11-10.0.26200-SP0, 16 logical CPUs
- Trials: 10, idle sample: 10.0 s
- Settings: TRANSCRIBE_MODE="streaming", LIVE_PASTE=False, AUTO_PASTE=True, model=nova-3
- Start time has a resolution of about 150 ms (one hotkey press per 150 ms).

| Metric | Result |
|---|---|
| Start time (launch until Alt+M works) | 1.56 s (min 1.23, max 1.56, n=3) |
| Idle memory (working set) | 62.6 MB |
| Idle memory (private) | 517.2 MB |
| Peak memory (working set, after trials) | 66.6 MB |
| Idle CPU | 0.31 % |
| Hotkey to meter | 37 ms (min 19, p90 48, max 48, n=10) |
| Start to first text | 10930 ms (min 10901, p90 10983, max 11066, n=9) |
| Stop to all text in place | 499 ms (min 471, p90 553, max 636, n=9) |
| Trials with the full text in the text box | 9 of 10 |
| Transcript accuracy (word match, median) | 100 % |
| Disk: app files | 0.04 MB |
| Disk: runtime (Python and packages) | 199.6 MB |

First transcript:

> This is a benchmark for the dictation app it measures how long it takes from the moment you stop speaking until the text appears at the cursor
