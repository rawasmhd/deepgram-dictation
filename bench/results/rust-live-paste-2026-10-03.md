# Benchmark: rust-live-paste-2026-10-03

- Date: 2026-10-03 12:17
- App: rust, commit `f9bf061`
- Machine: Windows-11-10.0.26200-SP0, 16 logical CPUs
- Trials: 10, idle sample: 10.0 s
- Settings: streaming, live paste on
- Start time has a resolution of about 150 ms (one hotkey press per 150 ms).

| Metric | Result |
|---|---|
| Start time (launch until Alt+M works) | 0.15 s (min 0.15, max 0.15, n=3) |
| Idle memory (working set) | 14.3 MB |
| Idle memory (private) | 3.4 MB |
| Peak memory (working set, after trials) | 16.5 MB |
| Idle CPU | 0.00 % |
| Hotkey to meter | 7 ms (min 6, p90 9, max 13, n=10) |
| Start to first text | 3349 ms (min 3117, p90 5011, max 8756, n=10) |
| Stop to all text in place | 0 ms (min 0, p90 0, max 0, n=10) |
| Paste complete (text box = the app's transcript) | 9 of 10 |
| Exact transcript (text box = sample.txt) | 9 of 10 |
| Transcript accuracy (word match, median) | 100 % |
| Disk: app files | 1.02 MB |
| Disk: runtime (Python and packages) | 0.0 MB |

First transcript:

> This is a benchmark for the dictation app it measures how long it takes from the moment you stop speaking until the text appears at the cursor
