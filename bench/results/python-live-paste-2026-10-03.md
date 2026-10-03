# Benchmark: python-live-paste-2026-10-03

- Date: 2026-10-03 12:14
- App: python, commit `f9bf061`
- Machine: Windows-11-10.0.26200-SP0, 16 logical CPUs
- Trials: 10, idle sample: 10.0 s
- Settings: TRANSCRIBE_MODE="streaming", LIVE_PASTE=True, AUTO_PASTE=True, model=nova-3
- Start time has a resolution of about 150 ms (one hotkey press per 150 ms).

| Metric | Result |
|---|---|
| Start time (launch until Alt+M works) | 0.77 s (min 0.77, max 1.08, n=3) |
| Idle memory (working set) | 62.9 MB |
| Idle memory (private) | 517.3 MB |
| Peak memory (working set, after trials) | 67.6 MB |
| Idle CPU | 0.62 % |
| Hotkey to meter | 31 ms (min 18, p90 50, max 52, n=10) |
| Start to first text | 3021 ms (min 2909, p90 3264, max 3275, n=10) |
| Stop to all text in place | 0 ms (min 0, p90 644, max 1202, n=10) |
| Paste complete (text box = the app's transcript) | 10 of 10 |
| Exact transcript (text box = sample.txt) | 10 of 10 |
| Transcript accuracy (word match, median) | 100 % |
| Disk: app files | 0.04 MB |
| Disk: runtime (Python and packages) | 199.6 MB |

First transcript:

> This is a benchmark for the dictation app it measures how long it takes from the moment you stop speaking until the text appears at the cursor
