# Voice profile: keep only your voice before Deepgram

Use this file as the reference for the "voice profile" idea (#64): the app learns the user's voice once,
and then passes only that voice to Deepgram, so other people in a shared room do not end up in the text.
Researched on 2026-11-02. Check the [sources](#sources) again before you rely on a number.

Two different techniques hide behind the words "voice profile". This page covers both, what other products do,
which models exist, what our Rust build can run today, and what to do next.

## Summary

- **It is possible, but it is not "a few MB and pure Rust".** The smallest usable speaker models are 25–30 MB ONNX files,
  and the only public model that removes a second talker from a live stream is 74 MB and uses about one CPU core.
- **There are two techniques.** A *speaker verification gate* decides every 1–2 s "is this me?" and mutes the rest.
  *Target speaker extraction* removes other voices from the mixture, even when two people talk at the same time.
  Teams and Zoom do the second one, with a 30 s read-aloud profile, but only inside their own clients.
- **Our build cannot run any model today.** The GNU toolchain has no working `dlltool.exe`, so every Rust inference crate
  (`ort`, `tract`, `candle`, `burn`, `sherpa-onnx`) fails to build. Tested on this machine. The MSVC move (#11) comes first.
- **Deepgram cannot do it for us.** It has no voice enrolment. Streaming diarization (`diarize_model=latest`, +$0.002/min)
  labels the speakers after the fact, which lets us drop other people's words, but not when they overlap with yours.
- **Cheap wins come first.** A headset or a close microphone removes most of the problem for free.
  Diarization is a one-parameter change. A proper voice profile is a multi-ticket project.

What a voice profile can and cannot fix:

| Situation | Speaker verification gate | Target speaker extraction | Deepgram diarization |
|---|---|---|---|
| Other people talk while you are silent | Good: audio is muted | Good | Good: words dropped |
| Other people talk at the same time as you | Poor: the window is a blend, it drops your words or passes theirs | Partial: about half the extra errors go away on small models | No: Deepgram already heard the mixture |
| Background noise (keyboards, air conditioning) | No effect | Partial | No effect, Nova-3 is noise-tolerant anyway |
| First word of a sentence | Late by 1–2 s, or sent before the decision | 24–100 ms delay | No effect |

## What other products call a voice profile

Most "voice profile" features are not filters. They identify the speaker to pick an account, or they adapt the recognizer to the user.

| Product | What you enrol | What the profile does | Filters other voices? |
|---|---|---|---|
| Microsoft Teams voice isolation | Read a passage, "about 30 seconds", or let Teams learn during meetings. Stored on the device. | Removes "any sounds or voices during a call or meeting that don't match the profile" | **Yes.** Teams client only, no API. |
| Zoom personalized audio isolation | Automatic voiceprint from meetings, optional recorded script | Keeps your voice. Zoom says the recording is what makes it work on a laptop mic instead of a headset. | **Yes.** Zoom client only. |
| Apple personalised "Hey Siri" | 5 short phrases, then up to 40 accepted utterances | Checks that the trigger phrase is you. 100-dim vector, EER 4.3%, about 1 false accept a month at 5.2% false rejects. | Gate for one phrase only |
| Amazon Alexa voice ID, Google Voice Match | "Learn my voice" in the app | Picks whose calendar and results to use | No |
| Windows Speech Recognition training | Read text | Adapted the recognizer. Deprecated Dec 2023, replaced by voice access, which has no voice profile. | No |
| Nuance Dragon user profile | Optional reading, microphone setup per source | Acoustic and vocabulary adaptation. "No voice profile training required" in current marketing. | No |
| Krisp | Nothing | Keeps the loudest, closest speaker ("wear a headset, toggle Krisp ON") | Yes, no profile. Paid SDK with Rust bindings, price on request. |

Lessons for us: 20–30 s of reading is the industry norm, the profile stays on the device, and the two products that really filter other
voices (Teams, Zoom) both say a headset microphone is the easy case and a laptop microphone needs the enrolment recording.

## Technique A: speaker verification gate

This is the lightweight option. One model turns a window of speech into a vector (an *embedding*). Enrolment stores the average vector
of the user. At run time the app compares each new window with the stored vector (cosine similarity) and mutes windows below a threshold.

How it works in practice:

- **Features.** 16 kHz audio, 25 ms frames every 10 ms, 80-dim log mel filterbanks (Kaldi style). This is the same for WeSpeaker, 3D-Speaker and NeMo models.
- **One vector per window, not per 20 ms block.** The models pool over the whole window. Models are trained on 1.5–3 s crops. A 20 ms block has no speaker identity.
  sherpa-onnx's own example drops anything under 0.5 s.
- **Enrolment.** 5–30 s of speech (Resemblyzer), 3–5 utterances (sherpa-onnx demo), 5 phrases plus 40 accepted ones (Apple). Record it in the real room
  with the real microphone: Apple notes that clean-room enrolment "often contain[s] very little environmental variability".
- **Threshold.** No library default. sherpa-onnx examples use 0.5–0.6 cosine. It depends on the model, window length and room.
- **Cost.** CAM++ runs at real-time factor 0.013–0.023 on one CPU thread. That is about 20–35 ms per 1.5 s window, or 4–7% of one core with a 0.5 s hop.
- **Latency.** The gate cannot decide about the first word until 1–2 s of audio exists. Options: buffer and send late, send first and let the user delete,
  or use the gate only to *stop* streaming when a window is judged "not me".

Candidate models (all 16 kHz, 80-dim fbank, cosine scoring, ONNX from the sherpa-onnx `speaker-recongition-models` release unless noted):

| Model | Params | EER VoxCeleb1-O | ONNX size | Licence | Note |
|---|---|---|---|---|---|
| WeSpeaker CAM++ | 7.2 M | 0.80% | 29.3 MB | Apache-2.0 | Fastest (RTF 0.013). **First choice.** |
| WeSpeaker ECAPA-TDNN c512 | 6.2 M | 1.07% | 24.9 MB | Apache-2.0 | Smallest. Hugging Face only, needs sherpa's `add_meta_data.py` once. |
| WeSpeaker ResNet34 | 6.6 M | 0.87% | 26.5 MB | Apache-2.0 | 3x slower than CAM++ |
| 3D-Speaker CAM++ (EN) | 7.2 M | 0.73% | 29.6 MB | Toolkit Apache-2.0, model licence not stated | Expects [-1, 1] samples and mean normalisation |
| 3D-Speaker ERes2Net-base (EN) | 6.6 M | 0.97% | 26.5 MB | As above | |
| NVIDIA TitaNet-S | 6.4 M | 1.15% | 40.3 MB | NeMo licence | |
| SpeechBrain ECAPA-TDNN | 20.8 M | 0.80% | No official ONNX (83 MB PyTorch) | Apache-2.0 | |
| Resemblyzer (GE2E d-vector) | small LSTM | Not published | No ONNX | Apache-2.0 | 1.6 s windows, the classic "voice profile" library |

EER (equal error rate) is where false accepts equal false rejects, measured on YouTube interview clips of several seconds. It is not the error
rate you get in a shared office. The 3D-Speaker README shows the same models going from 0.5–1% EER on VoxCeleb to 6–9% on a multi-device,
multi-distance test set. Expect the worse number with a laptop microphone.

Weak points for dictation:

1. **Overlap.** A window with two voices gives a blended vector. The gate either drops your words or passes theirs. It cannot separate them.
2. **Short windows.** Published EERs are for multi-second clips. Sub-second windows are much worse.
3. **Microphone or room change** costs about 10x in EER. Re-enrol when the setup changes.
4. **A false reject deletes your words.** For dictation that is worse than a false accept (an extra sentence you delete). Use a loose threshold,
   smooth over consecutive windows, use the gate to stop rather than start, and show an indicator when audio is being dropped.

## Technique B: target speaker extraction

This is what Teams and Zoom do. A model takes the mixture and the user's embedding, and outputs audio with only the user's voice.
It handles overlap, but it is heavier, and every causal model has the same characteristic failure: *target speaker over-suppression*,
where the filter deletes the user's own words for short stretches (2–14% of frames in Microsoft's paper).

| Model (year) | Size | CPU | Streaming? | Weights | Result on overlapped speech |
|---|---|---|---|---|---|
| VoiceFilter-Lite (Google, 2020) | 2.2 MB, 8-bit | Real time on a phone | Yes | **None public** | WER 42.1% → 28.8–31.4% (clean 15.2%). Enhances Google's ASR features, no audio output. |
| Personalized PercepNet (Amazon, 2021) | 8.5 M params | 4.7% of one i7 core | Yes, 10 ms frames | **None public** | PESQ 1.46 → 2.36. Pitch artefacts when talkers overlap. |
| pDCCRN (Microsoft, 2021) | Not stated | Real time on PC | Yes, causal | **None public** | WER 36.4% → 25–29% |
| DNS Challenge personalized track winners (2022–23) | 7.8–12.4 M | RTF 0.19–0.96 on i5 | Yes, ≤20–40 ms | Not released | Over-suppression hurt word accuracy enough that the organisers added it to the metric |
| SpEx+ (2020) | 11.1 M | Offline | **No** | Third-party | SI-SDR 2.5 → 18.2 dB in the lab. Same-gender pairs 3.3 dB worse. |
| **VoxLock (Oct 2026)** | 27 M, ONNX 74 MB int8 | 61–110% of one thread at 80–20 ms chunks | Yes, 24 ms latency | **Apache-2.0**, GitHub, Hugging Face, sherpa-onnx | WER 23.2% → 4.95% (Krisp benchmark), 42.95% → 10.63% (AMI meetings). Author-reported. Enrols from the first 3 s of speech. |
| OpenSpeakerBeam-SS, tse-conv-tasnet-48k (hobby, 2025–26) | 5–8 MB | Not measured | Yes | MIT / unverified | Weak or unpublished quality |

Bottom line: no small open model with proven quality exists. VoxLock is the only credible public streaming option, and it costs a CPU core and 74 MB.

Not a substitute: noise suppressors. RNNoise (C, BSD), DeepFilterNet 3 (Rust crate `deep_filter`, MIT/Apache, RTF 0.19, 48 kHz, 8 MB model) and
Silero VAD (2 MB, MIT) remove noise or silence. A second talker is speech and passes straight through.

## What Deepgram offers

| Feature | Helps? | Cost | Note |
|---|---|---|---|
| `diarize_model=latest` | Partly | +$0.0020/min streaming on top of $0.0077/min | A `speaker` number per word, also in streaming. The app can keep only one speaker's words. Labels are per connection and anonymous: the app must guess which one is "me", for example the speaker who dominates the first second after the key goes down. Does nothing for overlapped words, which were already misrecognised. `diarize=true` is deprecated; the v2 diarizer is batch only. |
| `multichannel` | No | Billed per channel | Only for physically separate microphones. |
| Voice enrolment, speaker identification | Not offered | | |
| Nova-3 | Already | | Recommended by Deepgram for "multi-speaker, noisy, or far-field audio". No noise metric published. |

## Running a model in our Rust app

The hard constraint is not "no C compiler" but "no working `dlltool.exe`". rustup's GNU toolchain ships a linker-only gcc and a `dlltool.exe` that fails
because there is no assembler. Any crate with `#[link(kind = "raw-dylib")]` needs dlltool. That now includes `windows-sys` 0.61, `windows-link` 0.2,
`libloading` 0.9, `getrandom` 0.3+ (pulled in by `rand` 0.9+, so by every ML crate) and `ort-sys` itself. Our app builds only because `Cargo.lock`
holds `windows-sys` at 0.59 through the `schannel` pin.

Test builds on this machine (2026-11-02, `x86_64-pc-windows-gnu`, release):

| Route | Builds? | Size to ship | Note |
|---|---|---|---|
| `ort` 2.0.0-rc.15 with `load-dynamic` | **No**: dlltool | `onnxruntime.dll` 16.5 MiB | Works on MSVC. Loads the DLL at run time, no linking. Official Microsoft DLL, MIT. |
| Hand-written FFI over the ORT C API, zero dependencies | **Yes** (0.6 s) | Same DLL | Loaded both the official 1.31.1 DLL and the Windows copy. Unsafe code, hand-maintained struct layout. |
| `tract-onnx` 0.23.9 (pure Rust) | **No**: dlltool via `windows-sys` 0.61, then assembler for its x86 kernels | 5–10 MB in the exe | Production at Sonos, audio-focused |
| `candle` 0.11.1 + `candle-onnx` | **No**: dlltool via `getrandom` | 3–6 MB in the exe | Only ~80 ONNX ops |
| `burn` + `burn-onnx` | Not tested, same `getrandom` problem expected | | Build-time code generation |
| `sherpa-onnx` 2.0.1 official crate | **No**: prebuilt MSVC static libs | `sherpa-onnx-c-api.dll` + `onnxruntime.dll` | Speaker ID, Silero VAD, GTCRN denoiser and VoxLock in one crate. Best fit after the MSVC move. |
| `deep_filter` (DeepFilterNet) | **No**: tract | ~27 MB | |
| Windows' own `C:\Windows\System32\onnxruntime.dll` | Loadable | 0 | Version 1.17.1 here, undocumented for third parties, differs per machine. Curiosity only. |
| New Windows ML (Windows App SDK 1.8) | | 20–41 MB self-contained | "Framework-dependent C/C++ support is not currently available." |

Other parts are fine in pure Rust: 80-dim Kaldi fbank via `kaldi-native-fbank` 0.1.0 (Rust port, young) or 150 lines on `realfft`.

Consequences:

1. **The MSVC migration (#11) is the prerequisite** for any on-device model. It unlocks `ort`, `sherpa-onnx`, `tract`, `candle` at once.
   The dlltool wall will otherwise come back with every `windows-*` or `getrandom` bump.
2. **Single-exe ends.** ONNX Runtime is a 16.5 MiB DLL next to the exe, or downloaded at first start with a SHA-256 check.
3. **Total footprint of a verification gate:** DLL 16.5 MiB + CAM++ 29 MB + the app. About 5% of one core while recording.

## Windows built-in effects and this PC

| Effect | Needs | On this PC | Gets to our cpal stream? |
|---|---|---|---|
| Windows Studio Effects "Voice Focus" | NPU and OEM driver | **No NPU** (Ryzen 7 7730U) | Would be applied for all apps, but noise only |
| Voice Clarity (24H2, CPU) | Stream in the Communications signal-processing mode | Windows 11 25H2, so available | **No.** cpal never calls `IAudioClient2::SetClientProperties`, so our stream is "Default" mode. A ~20-line WASAPI change to set `AudioCategory_Communications` would enable it. Side effect: Windows may duck other audio. Noise only. |
| NVIDIA Broadcast | RTX GPU | **No** (AMD Radeon integrated) | |

This PC: AMD Ryzen 7 7730U, 8 cores, Realtek microphone array (laptop built-in). That is the hard case Teams and Zoom describe: a room microphone
hears everyone at about the same level. A headset or desk microphone close to the mouth changes the problem more than any model does.

## How well will it work

Honest numbers from the papers, for two people talking at the same time into one room microphone:

- Small causal models (2–10 MB) recover about a third to a half of the extra errors: roughly 25–30% WER left where clean speech gives 15%.
- A 27 M-param model (VoxLock) gets to 5–10% WER, author-reported, at one CPU core.
- Same-gender pairs are 3–4 dB worse in every paper. Far-field microphones are worst.
- Enrolment length saturates between 15 and 30 s. Longer does not help.
- A verification gate alone cannot fix overlap at all. It fixes the case where others talk while you pause.

## Recommendation

Do it in stages. Each stage is useful on its own, and each is its own ticket.

| Stage | What | Cost | Fixes | Blocked by |
|---|---|---|---|---|
| 0 | Headset or close microphone. Document it in the README. | Hardware | Most of it | Nothing |
| 1 | `diarize_model=latest` and keep only the dominant speaker's words. Setting in the tray menu. | +26% Deepgram price, one afternoon | Interjections while you pause | Nothing |
| 2 | Communications-category WASAPI stream for Windows Voice Clarity | Small patch to the capture code | Noise, not voices | Nothing |
| 3 | MSVC toolchain | #11 | Nothing by itself | |
| 4 | Speaker verification gate: CAM++ ONNX via `ort` `load-dynamic` or `sherpa-onnx`, 20–30 s enrolment in the setup window, 1.5 s windows with 0.5 s hop, hysteresis, gate stops streaming, indicator in the overlay | +46 MB on disk, ~5% of a core | Others talking while you pause, with no Deepgram cost | Stage 3 |
| 5 | Target speaker extraction with VoxLock (sherpa-onnx ships it) | +74 MB, one CPU core | Overlap | Stage 3, and a test that the quality holds on our clips |

Stage 1 is the only thing to build before the toolchain moves. Stage 4 is the "lightweight voice profile" the ticket asked for, and the closest an
open model gets today. Stage 5 is the Teams-style result, and it is not lightweight.

Before any model stage, record 10–20 clips in the real room (you alone, you with a colleague talking, a colleague alone) with the real microphone.
They are the test set for every threshold and model decision, like the clips in [stt-models.md](stt-models.md).

## Follow-up issues to open

- README: recommend a headset or close microphone for shared rooms (documentation).
- Deepgram streaming diarization: keep only the dominant speaker's words, with a setting (enhancement, stage 1).
- WASAPI Communications category so Windows Voice Clarity applies (enhancement, stage 2).
- Voice profile: enrolment and speaker verification gate (enhancement, stage 4, depends on #11).
- Spike: measure CAM++ embedding time on our hardware under `ort` once MSVC builds (enhancement, before stage 4).
- Target speaker extraction with VoxLock (enhancement, stage 5, after stage 4).

## Sources

Speaker verification

- [x-vectors paper (Snyder et al., 2018)](https://www.danielpovey.com/files/2018_icassp_xvectors.pdf)
- [ECAPA-TDNN paper](https://arxiv.org/abs/2005.07143), [CAM++ paper](https://arxiv.org/abs/2303.00332), [TitaNet paper](https://arxiv.org/abs/2110.04410), [GE2E paper](https://arxiv.org/abs/1710.10467)
- [WeSpeaker VoxCeleb recipe results](https://github.com/wenet-e2e/wespeaker/blob/master/examples/voxceleb/v2/README.md), [WeSpeaker ONNX runtime RTF](https://github.com/wenet-e2e/wespeaker/blob/master/runtime/onnxruntime/README.md), [WeSpeaker CAM++ on Hugging Face](https://huggingface.co/Wespeaker/wespeaker-voxceleb-campplus)
- [3D-Speaker README (cross-dataset EER table)](https://github.com/modelscope/3D-Speaker/blob/main/README.md)
- [sherpa-onnx speaker recognition models release](https://github.com/k2-fsa/sherpa-onnx/releases/tag/speaker-recongition-models), [speaker-embedding-manager.cc](https://github.com/k2-fsa/sherpa-onnx/blob/master/sherpa-onnx/csrc/speaker-embedding-manager.cc), [speaker-identification-with-vad.py](https://github.com/k2-fsa/sherpa-onnx/blob/master/python-api-examples/speaker-identification-with-vad.py), [Rust example](https://github.com/k2-fsa/sherpa-onnx/blob/master/rust-api-examples/examples/speaker_embedding_manager.rs), [sherpa-onnx crate](https://crates.io/crates/sherpa-onnx)
- [Resemblyzer](https://github.com/resemble-ai/Resemblyzer), [SpeechBrain ECAPA](https://huggingface.co/speechbrain/spkrec-ecapa-voxceleb), [TitaNet-L](https://huggingface.co/nvidia/speakerverification_en_titanet_large)
- [Apple: Personalized Hey Siri](https://machinelearning.apple.com/research/personalized-hey-siri)

Products

- [Teams voice isolation (admin)](https://learn.microsoft.com/microsoftteams/voice-isolation), [Teams voice isolation (user)](https://support.microsoft.com/office/voice-isolation-in-microsoft-teams-calls-and-meetings-a9756ea9-4cec-44c4-aefb-6f5d17c89427)
- [Zoom personalized audio isolation](https://support.zoom.com/hc/en/article?id=zm_kb&sysparm_article=KB0074698)
- [Google Voice Match](https://support.google.com/assistant/answer/9071681), [Alexa voice profiles (developer blog)](https://developer.amazon.com/en-US/blogs/alexa/post/1ad16e9b-4f52-4e68-9187-ec2e93faae55/recognize-voices-and-personalize-your-skill)
- [Windows deprecated features (WSR)](https://learn.microsoft.com/en-us/windows/whats-new/deprecated-features), [Dragon user profiles](https://www.nuance.com/products/help/dragon15/dragon-for-pc/enx/professionalindividual/Content/Users/about_user_profiles.htm)
- [Krisp developers](https://krisp.ai/developers/), [NVIDIA Broadcast FAQ](https://www.nvidia.com/en-gb/geforce/broadcasting/broadcast-app/faq)

Target speaker extraction and noise suppression

- [VoiceFilter](https://arxiv.org/abs/1810.04826), [VoiceFilter-Lite](https://arxiv.org/abs/2009.04323), [Personalized PercepNet](https://arxiv.org/abs/2106.04129), [Microsoft pDCCRN](https://arxiv.org/abs/2110.09625), [SpEx+](https://arxiv.org/abs/2005.04686)
- [ICASSP 2022 DNS Challenge](https://www.microsoft.com/en-us/research/academic-program/deep-noise-suppression-challenge-icassp-2022/), [DNS 2022 paper](https://arxiv.org/abs/2202.13288), [ICASSP 2023 DNS Challenge](https://www.microsoft.com/en-us/research/academic-program/deep-noise-suppression-challenge-icassp-2023/)
- [VoxLock paper](https://arxiv.org/abs/2610.40727), [VoxLock repo](https://github.com/arbazsiddiqui/voxlock), [WeSep](https://github.com/wenet-e2e/wesep)
- [RNNoise paper](https://arxiv.org/abs/1709.08243), [DeepFilterNet repo](https://github.com/Rikorose/DeepFilterNet), [DeepFilterNet3 paper](https://arxiv.org/abs/2305.08227), [Silero VAD](https://github.com/snakers4/silero-vad)

Deepgram

- [Diarization](https://developers.deepgram.com/docs/diarization), [Streaming feature overview](https://developers.deepgram.com/docs/stt-streaming-feature-overview), [Multichannel](https://developers.deepgram.com/docs/multichannel), [Pricing](https://deepgram.com/pricing), [Streaming latency](https://developers.deepgram.com/docs/measuring-streaming-latency)

Rust and Windows

- [ort linking docs](https://ort.pyke.io/setup/linking), [ort multi-version](https://github.com/pykeio/ort/blob/main/docs/content/setup/multiversion.mdx), [ort issue 331 (windows-gnu)](https://github.com/pykeio/ort/issues/331), [ort issue 561](https://github.com/pykeio/ort/issues/561), [ort issue 583 (System32 DLL)](https://github.com/pykeio/ort/issues/583)
- [ONNX Runtime 1.31.1 release](https://github.com/microsoft/onnxruntime/releases/tag/v1.31.1), [ONNX Runtime on Windows](https://onnxruntime.ai/docs/get-started/with-windows.html)
- [Windows ML overview](https://learn.microsoft.com/en-us/windows/ai/new-windows-ml/overview), [Windows ML distribution](https://learn.microsoft.com/en-us/windows/ai/new-windows-ml/distributing-your-app)
- [getrandom Windows backend (raw-dylib)](https://docs.rs/crate/getrandom/0.4.3/source/src/backends/windows.rs), [windows-link 0.2 source](https://docs.rs/crate/windows-link/0.2.1/source/src/lib.rs)
- [kaldi-native-fbank Rust port](https://crates.io/crates/kaldi-native-fbank), [tract](https://github.com/sonos/tract), [candle](https://github.com/huggingface/candle), [burn](https://github.com/tracel-ai/burn)
- [Windows Studio Effects](https://learn.microsoft.com/en-us/windows/apps/develop/windows-integration/studio-effects), [Voice Clarity announcement (build 26040)](https://blogs.windows.com/windows-insider/2024/01/26/announcing-windows-11-insider-preview-build-26040-canary-channel/), [Audio signal processing modes](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/audio-signal-processing-modes), [IAudioClient2::SetClientProperties](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclient2-setclientproperties), [cpal WASAPI device.rs](https://github.com/RustAudio/cpal/blob/master/src/host/wasapi/device.rs)
