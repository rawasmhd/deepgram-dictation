# Benchmark

This folder measures the app with the same, repeatable measurements (issue #16). It was used to compare the old Python version with the Rust version; see [results/comparison-2026-10-03.md](results/comparison-2026-10-03.md).

`bench.py` starts `dictation.exe` as a separate process, presses the hotkeys with `SendInput`, and watches the app's windows and a target text box. The text box is a standard Windows `EDIT` control. A Tkinter text box dropped some pastes, so the benchmark does not use one (#18). The app gets `sample.wav` as its microphone (`DICTATION_FAKE_AUDIO`), so every trial hears the same speech.

## Metrics

| Metric | How it is measured |
|---|---|
| Start time | From launch until Alt+M shows the meter. The script presses Alt+M every 150 ms, so the resolution is about 150 ms. Median of 3 launches. |
| Idle memory | Working set and private memory, 3 seconds after start. |
| Idle CPU | CPU time used during 10 seconds of waiting. |
| Hotkey to meter | From Alt+M until the meter window is visible. |
| Start to first text | From Alt+M until the first text arrives in the text box. Only useful with live paste. |
| Stop to all text in place | From the second Alt+M until the last change to the text box. 0 ms means that all text was already in place at the stop. **This is the most important metric.** |
| Paste complete | The text box has exactly the text that the app wrote to its log (`-> text`). If this fails, text was lost during the paste. |
| Exact transcript | The text box has exactly the words of `sample.txt`. If this fails but "paste complete" passes, Deepgram returned different words. |
| Transcript accuracy | Word match with `sample.txt` (median). |
| Disk | The size of `dictation.exe`. It needs no runtime. |

Words are compared without punctuation and case. The script presses the second Alt+M 0.5 seconds after the end of `sample.wav`.

## Run

Build the app first (`cargo build --release` in `rust/`), and quit any running copy (Ctrl+Alt+Q). Do not use the keyboard or mouse while the benchmark runs: the script needs the focus on its text box. If you close the text box, the run stops and saves nothing.

```bash
python bench/bench.py
```

Options:

- `--mode streaming` (default) or `--mode batch`.
- `--live-paste on` (default) or `--live-paste off`. Only for streaming.
- `--exe PATH` tests another `dictation.exe` (default: `rust/target/release/dictation.exe`).
- `--trials N` (default 10), `--startups N` (default 3), `--idle-seconds N` (default 10).
- `--label NAME` names the results files.

Each trial sends about 10 seconds of audio to Deepgram, so a run uses a little API credit. The app writes its log to `bench/bench.log` during a run.

The results go to `results/` as JSON (all values) and Markdown (a summary).

## Fair comparison

- Use the same settings in both versions (live paste, streaming or batch).
- Run both versions on the same day, on the same PC and network. Network speed changes the "stop to all text" result.
- Close other heavy apps.

## Files

- `bench.py`: the benchmark.
- `sample.wav`, `sample.txt`: the test speech (Windows text-to-speech, 16 kHz, mono, about 10 seconds) and its text.
- `results/`: earlier results, including the Python version's baseline.
