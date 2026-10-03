# Benchmark: python-baseline-batch

- Date: 2026-10-02 22:14
- App: python, commit `269890c`
- Machine: Windows-11-10.0.26200-SP0, 16 logical CPUs
- Trials: 10, idle sample: 10.0 s
- Settings: TRANSCRIBE_MODE='batch', LIVE_PASTE=False, AUTO_PASTE=True, model=nova-3
- Start time has a resolution of about 150 ms (one hotkey press per 150 ms).

| Metric | Result |
|---|---|
| Start time (launch until Alt+M works) | 0.77 s (min 0.77, max 0.92, n=3) |
| Idle memory (working set) | 58.9 MB |
| Idle memory (private) | 516.3 MB |
| Peak memory (working set, after trials) | 65.4 MB |
| Idle CPU | 0.62 % |
| Hotkey to meter | 25 ms (min 9, p90 37, max 38, n=10) |
| Start to first text | 13681 ms (min 13091, p90 20466, max 20664, n=10) |
| Stop to all text in place | 3250 ms (min 2661, p90 10036, max 10234, n=10) |
| Paste complete (text box = the app's transcript) | 10 of 10 |
| Exact transcript (text box = sample.txt) | 10 of 10 |
| Transcript accuracy (word match, median) | 100 % |
| Disk: app files | 0.04 MB |
| Disk: runtime (Python and packages) | 199.6 MB |

First transcript:

> This is a benchmark for the dictation app it measures how long it takes from the moment you stop speaking until the text appears at the cursor
