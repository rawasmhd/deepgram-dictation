# Benchmark: python-baseline-batch

- Date: 2026-10-01 16:45
- App: python, commit `7c40ed3`
- Machine: Windows-11-10.0.26200-SP0, 16 logical CPUs
- Trials: 10, idle sample: 10.0 s
- Settings: TRANSCRIBE_MODE='batch', LIVE_PASTE=False, AUTO_PASTE=True, model=nova-3
- Start time has a resolution of about 150 ms (one hotkey press per 150 ms).

| Metric | Result |
|---|---|
| Start time (launch until Alt+M works) | 1.40 s (min 1.24, max 1.40, n=3) |
| Idle memory (working set) | 59.2 MB |
| Idle memory (private) | 516.6 MB |
| Peak memory (working set, after trials) | 65.3 MB |
| Idle CPU | 0.62 % |
| Hotkey to meter | 41 ms (min 14, p90 51, max 52, n=10) |
| Start to first text | 12607 ms (min 12452, p90 12857, max 13029, n=9) |
| Stop to all text in place | 2177 ms (min 2021, p90 2426, max 2599, n=9) |
| Trials with the full text in the text box | 9 of 10 |
| Transcript accuracy (word match, median) | 100 % |
| Disk: app files | 0.04 MB |
| Disk: runtime (Python and packages) | 199.6 MB |

First transcript:

> This is a benchmark for the dictation app it measures how long it takes from the moment you stop speaking until the text appears at the cursor
