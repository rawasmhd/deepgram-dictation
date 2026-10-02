# Benchmark: python-baseline-live-paste

- Date: 2026-10-01 16:40
- App: python, commit `7c40ed3`
- Machine: Windows-11-10.0.26200-SP0, 16 logical CPUs
- Trials: 10, idle sample: 10.0 s
- Settings: TRANSCRIBE_MODE="streaming", LIVE_PASTE=True, AUTO_PASTE=True, model=nova-3
- Start time has a resolution of about 150 ms (one hotkey press per 150 ms).

| Metric | Result |
|---|---|
| Start time (launch until Alt+M works) | 1.40 s (min 1.24, max 1.55, n=3) |
| Idle memory (working set) | 62.7 MB |
| Idle memory (private) | 517.4 MB |
| Peak memory (working set, after trials) | 67.6 MB |
| Idle CPU | 1.87 % |
| Hotkey to meter | 37 ms (min 14, p90 48, max 49, n=10) |
| Start to first text | 2768 ms (min 2723, p90 5431, max 9768, n=10) |
| Stop to all text in place | 0 ms (min 0, p90 0, max 0, n=10) |
| Trials with the full text in the text box | 7 of 10 |
| Transcript accuracy (word match, median) | 100 % |
| Disk: app files | 0.04 MB |
| Disk: runtime (Python and packages) | 199.6 MB |

First transcript:

>  until the text appears at the cursor
