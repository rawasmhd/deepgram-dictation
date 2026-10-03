# Speech-to-text models: baseline and comparisons

Use this file to check a new speech-to-text (STT) model against the one we use now.
Add each new candidate to the [log](#candidate-log) at the end.

WER (word error rate) is the share of words the model gets wrong. Lower is better.

## Our baseline: Deepgram Nova-3

| Item | Value |
|---|---|
| Model | Deepgram `nova-3`, cloud API |
| Mode | Streaming over WebSocket (default). Batch upload is the fallback. |
| Language | `en` (can change in `PARAMS`) |
| Options we use | `smart_format`, `punctuate`, `dictation`, `filler_words=false` |
| Cost | $0.0077/min streaming, $0.0043/min batch |
| Needs | Internet and a Deepgram API key |

Config: [rust/src/deepgram.rs](../rust/src/deepgram.rs) (`PARAMS`, `STREAM_PARAMS`). The `DICTATION_MODE` environment variable selects batch mode.

### Published accuracy

| Measure | Nova-3 | Who measured | Notes |
|---|---|---|---|
| Median WER, batch | 5.26% | Deepgram | 2,703 files, 81.7 h, 10 domains |
| Median WER, streaming | 6.84% | Deepgram | Same set. **We use streaming.** |
| Artificial Analysis WER Index | 5.2% | Artificial Analysis | 50% AgentTalk, 25% VoxPopuli, 25% Earnings-22 |
| Artificial Analysis speed | 564x real time | Artificial Analysis | |
| LibriSpeech clean / other | 2.5% / 4.8% | Third-party blog | Low trust. Confirm before you rely on it. |

Use the same benchmark for both models. Nova-3 has no score on the Open ASR Leaderboard,
because the leaderboard tests only open models. The best shared measure is the
**Artificial Analysis WER Index**.

Other models on the same index, for reference (checked 2026-10-01):

| Model | AA WER Index | Speed | Price / 1,000 min |
|---|---|---|---|
| Deepgram Nova-3 | 5.2% | 564x | $4.30 |
| Whisper Large v3 (fal.ai) | 4.1% | 113x | $1.15 |
| NVIDIA Parakeet TDT 0.6B v2 | 6.4% | 99x | $0 (local) |

## Requirements for a replacement

A new model must pass all of the "Must" checks. Otherwise we do not switch.

| Check | Level | Why |
|---|---|---|
| Accuracy equal to or better than Nova-3 on the same benchmark | Must | Main goal |
| Runs on Windows | Must | Main platform |
| Text ready in about 1 s or less after you stop talking | Must | Streaming gives this now |
| Punctuation and capitalization | Must | `punctuate`, `smart_format` |
| Spoken punctuation ("comma" → ",") | Should | `dictation` option |
| No filler words ("um", "uh") | Should | `filler_words=false` |
| Other languages | Should | Now one `PARAMS` change |
| Custom words (key terms) | Nice | Nova-3 accepts up to 100 |
| Local / offline, no API key, no cost | Nice | Privacy and cost |

## How to compare a new model

1. Find its score on the **Artificial Analysis WER Index**. Compare it with the tables above.
2. If it is a local model and has no AA score, compare Open ASR Leaderboard sets.
   Use the sets that also have a Nova-3 score, for example VoxPopuli and Earnings-22.
3. Go through the requirements table.
4. If it passes, test both models on the same 10–20 of our own dictation clips.
   Count the errors by hand or with a WER tool.
5. Add a row to the log below.

## Candidate log

| Date | Model | Type | Accuracy | Result | Reason |
|---|---|---|---|---|---|
| 2026-10-01 | Fermion Research Phonon-2 | Local, 164 MB, CC-BY-4.0, based on Parakeet TDT 0.6B v3 | 5.21% avg on Open ASR (7 English sets, self-reported). VoxPopuli 2.46%, Earnings-22 6.96%. | Not adopted | English only. No documented streaming or spoken punctuation. Windows backend not documented. No independent scores. Look again if it gets an AA score or a streaming mode. |

## Sources

- [Deepgram: Introducing Nova-3](https://deepgram.com/learn/introducing-nova-3-speech-to-text-api)
- [Artificial Analysis: Deepgram models](https://artificialanalysis.ai/speech-to-text/models/deepgram)
- [How accurate is Deepgram? (third-party)](https://vexascribe.com/how-accurate-is-deepgram)
- [Phonon-2 on Hugging Face](https://huggingface.co/FermionResearch/Phonon-2)
