# Python baseline, 2026-10-02

The Python version on `main` (`dictate.py` as of `7c40ed3`), measured with `bench.py` on Windows 11 25H2, 16 logical CPUs. Every run used 10 trials and pasted into a standard Windows text box. The full values are in the JSON files next to this page.

| Metric | Live paste (default) | Streaming | Batch |
|---|---|---|---|
| Start time (launch until Alt+M works, median) | 0.93 s | 0.92 s | 0.77 s |
| Idle memory (working set) | 62.6 MB | 62.8 MB | 58.9 MB |
| Idle memory (private) | 517 MB | 517 MB | 516 MB |
| Peak memory (working set) | 67.8 MB | 67.7 MB | 65.4 MB |
| Idle CPU | 0.78 % | 1.25 % | 0.62 % |
| Hotkey to meter (median) | 19 ms | 31 ms | 25 ms |
| Start to first text (median) | 3.14 s | 10.95 s | 13.68 s |
| **Stop to all text in place (median)** | **0 ms** | **523 ms** | **3250 ms** |
| Stop to all text in place (p90) | 0 ms | 740 ms | 10036 ms |
| Paste complete | 9 of 10 | 10 of 10 | 10 of 10 |
| Exact transcript | 9 of 10 | 10 of 10 | 10 of 10 |
| Disk: app + runtime | 0.04 + 200 MB | same | same |

Settings: live paste = `LIVE_PASTE=True` (the default on `main`). Streaming = `LIVE_PASTE=False`. Batch = `LIVE_PASTE=False`, `TRANSCRIBE_MODE='batch'`. All use `nova-3`.

## Notes

- **One paste was incomplete (#18).** In live-paste trial 6, the last phrase ("until the text appears at the cursor") did not arrive, but the app logged it. A possible cause: after the last live paste, `finish_live()` puts the full text on the clipboard at once, while the target app can still be reading the clipboard for the Ctrl+V. This is not proven.
- **An earlier run lost more text** (5 of 30 trials), but it pasted into a Tkinter text box. A Notepad test then lost no text in 40 pastes. So the benchmark now uses a standard Windows text box.
- **Slow outliers come from the network.** One streaming trial took 9.0 s, because the stream returned no text and the app used the batch fallback. In batch mode, 2 trials took about 10 s. Compare the Rust version on the same day.
- **Idle memory (private) is high.** About 517 MB of memory is reserved, but only about 60 MB is in use. Numpy and its maths libraries probably reserve this memory.
- **Start time** has a resolution of about 150 ms. A cold start after a long pause can be slower (5.1 s in one test run).
