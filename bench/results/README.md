# Python baseline, 2026-10-01

The Python version at commit `7c40ed3` (`main` after #6), measured with `bench.py` on Windows 11 25H2, 16 logical CPUs. Every run used 10 trials. The full values are in the JSON files next to this page.

| Metric | Live paste (default) | Streaming | Batch |
|---|---|---|---|
| Start time (launch until Alt+M works) | 1.40 s | 1.56 s | 1.40 s |
| Idle memory (working set) | 62.7 MB | 62.6 MB | 59.2 MB |
| Idle memory (private) | 517 MB | 517 MB | 517 MB |
| Peak memory (working set) | 67.6 MB | 66.6 MB | 65.3 MB |
| Idle CPU | 1.87 % | 0.31 % | 0.62 % |
| Hotkey to meter (median) | 37 ms | 37 ms | 41 ms |
| Start to first text (median) | 2.77 s | 10.93 s | 12.61 s |
| **Stop to all text in place (median)** | **0 ms** | **499 ms** | **2177 ms** |
| Trials with the full text in the text box | 7 of 10 | 9 of 10 | 9 of 10 |
| Disk: app + runtime | 0.04 + 200 MB | same | same |

Settings: live paste = `LIVE_PASTE=True` (the default on `main`). Streaming = `LIVE_PASTE=False`. Batch = `LIVE_PASTE=False`, `TRANSCRIBE_MODE='batch'`. All use `nova-3`.

## Notes

- **Text lost during the paste.** Deepgram returned the full, correct text in all 30 trials (`dictation.log` has 30 complete transcripts). But some text did not arrive in the text box. With live paste, one phrase was missing in 3 trials. In streaming and batch mode, nothing was pasted in 1 trial each. The cause is not known yet. It can be in the app (for example, the Alt key releases before Ctrl+V) or in the benchmark's text box.
- **Idle memory (private) is high.** About 517 MB of memory is reserved, but only about 60 MB is in use. Numpy and its maths libraries probably reserve this memory.
- **Idle CPU** is low in all modes. The differences between the runs are probably noise.
- **Start time** has a resolution of about 150 ms. The first start after a long pause can be slower (5.1 s in a test run), because Windows has not cached the files yet.
- **Stop to all text** depends on the network and on Deepgram. Compare the Rust version on the same day.
