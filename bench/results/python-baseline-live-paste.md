# Benchmark: python-baseline-live-paste

- Date: 2026-10-02 22:09
- App: python, commit `269890c`
- Machine: Windows-11-10.0.26200-SP0, 16 logical CPUs
- Trials: 10, idle sample: 10.0 s
- Settings: TRANSCRIBE_MODE="streaming", LIVE_PASTE=True, AUTO_PASTE=True, model=nova-3
- Start time has a resolution of about 150 ms (one hotkey press per 150 ms).

| Metric | Result |
|---|---|
| Start time (launch until Alt+M works) | 0.93 s (min 0.77, max 0.93, n=3) |
| Idle memory (working set) | 62.6 MB |
| Idle memory (private) | 517.2 MB |
| Peak memory (working set, after trials) | 67.8 MB |
| Idle CPU | 0.78 % |
| Hotkey to meter | 19 ms (min 11, p90 42, max 53, n=10) |
| Start to first text | 3137 ms (min 2881, p90 3670, max 4172, n=10) |
| Stop to all text in place | 0 ms (min 0, p90 0, max 0, n=10) |
| Paste complete (text box = the app's transcript) | 9 of 10 |
| Exact transcript (text box = sample.txt) | 9 of 10 |
| Transcript accuracy (word match, median) | 100 % |
| Disk: app files | 0.04 MB |
| Disk: runtime (Python and packages) | 199.6 MB |

First transcript:

> This is a benchmark for the dictation app it measures how long it takes from the moment you stop speaking until the text appears at the cursor
