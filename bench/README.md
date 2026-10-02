# Benchmark

This folder compares versions of the app with the same, repeatable measurements (issue #16).

`bench.py` starts the app as a separate process, presses the hotkeys with `SendInput`, and watches the app's windows and a target text box. The text box is a standard Windows `EDIT` control. A Tkinter text box dropped some pastes, so the benchmark does not use one (#18). The app gets `sample.wav` as its microphone, so every trial hears the same speech.

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

Words are compared without punctuation and case.
| Disk | The app files, plus the runtime that they need (Python and the packages). |

The script presses the second Alt+M 0.5 seconds after the end of `sample.wav`.

## Run

Stop any running copy of the app first (`scripts\Stop Dictation.bat`). Do not use the keyboard or mouse while the benchmark runs: the script needs the focus on its text box.

```bash
python bench/bench.py --app python
```

Options:

- `--set NAME=VALUE` changes a setting at the top of `dictate.py` for the run, for example `--set LIVE_PASTE=False` or `--set "TRANSCRIBE_MODE='batch'"`.
- `--trials N` (default 10), `--startups N` (default 3), `--idle-seconds N` (default 10).
- `--label NAME` names the results files.

Each trial sends about 10 seconds of audio to Deepgram, so a run uses a little API credit.

The results go to `results/` as JSON (all values) and Markdown (a summary).

### Rust version

```bash
python bench/bench.py --app rust --exe rust/target/release/dictation.exe --settings "live paste on, streaming"
```

The script sets `DICTATION_FAKE_AUDIO` to the path of `sample.wav`. The Rust app must then read this file instead of the microphone. This is part of issue #8. Until then, use `--no-transcribe`.

For the "paste complete" metric, give the Rust app's log file with `--log PATH`. The log must have one `-> text` line for each dictation, like `dictate.py`.

## Fair comparison

- Use the same settings in both versions (live paste, streaming or batch, model).
- Run both versions on the same day, on the same PC and network. Network speed changes the "stop to all text" result.
- Close other heavy apps.

## Files

- `bench.py`: the benchmark.
- `run_python.py`: starts `dictate.py` with `sample.wav` as the microphone. Nothing else in `dictate.py` changes.
- `sample.wav`, `sample.txt`: the test speech (Windows text-to-speech, 16 kHz, mono, about 10 seconds) and its text.
