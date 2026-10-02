# Benchmark: python-baseline-streaming

- Date: 2026-10-02 22:11
- App: python, commit `269890c`
- Machine: Windows-11-10.0.26200-SP0, 16 logical CPUs
- Trials: 10, idle sample: 10.0 s
- Settings: TRANSCRIBE_MODE="streaming", LIVE_PASTE=False, AUTO_PASTE=True, model=nova-3
- Start time has a resolution of about 150 ms (one hotkey press per 150 ms).

| Metric | Result |
|---|---|
| Start time (launch until Alt+M works) | 0.92 s (min 0.77, max 0.92, n=3) |
| Idle memory (working set) | 62.8 MB |
| Idle memory (private) | 517.2 MB |
| Peak memory (working set, after trials) | 67.7 MB |
| Idle CPU | 1.25 % |
| Hotkey to meter | 31 ms (min 10, p90 50, max 50, n=10) |
| Start to first text | 10954 ms (min 10933, p90 11170, max 19450, n=10) |
| Stop to all text in place | 523 ms (min 502, p90 740, max 9020, n=10) |
| Paste complete (text box = the app's transcript) | 10 of 10 |
| Exact transcript (text box = sample.txt) | 10 of 10 |
| Transcript accuracy (word match, median) | 100 % |
| Disk: app files | 0.04 MB |
| Disk: runtime (Python and packages) | 199.6 MB |

First transcript:

> This is a benchmark for the dictation app it measures how long it takes from the moment you stop speaking until the text appears at the cursor
