# Product shots for the README

The images in `assets/` that show the app are built here with the [stageshow](https://skills.sh/benjitaylor/skills) skill, not screenshots. Each demo is a small React scene in `examples/`, built to the skill's framing, craft and motion rules, in the app's own colours (`examples/dictation-shared/look.css`, from `rust/src/setup.rs` and `rust/src/overlay.rs`).

| Demo | Output | What it shows |
|---|---|---|
| `dictation-hero` | `assets/dictation-hero.gif` | One dictation: Alt+M, the meter, phrases pasted into a note, Transcribing, undo |
| `dictation-settings` | `assets/dictation-settings.png` | The Settings window |
| `dictation-words` | `assets/dictation-words.png` | The Custom words window |
| `dictation-tray` | `assets/dictation-tray.png` | The tray menu |
| `dictation-paused` | `assets/dictation-paused.png` | The meter while another window has focus |

`examples/dictation-shared/` holds the parts the demos share: the look, the Windows 11 window frame, the app icon from `rust/src/art.rs`, the taskbar, the keycaps and the meter.

## Film

The demos are built inside the skill's own preview, so they import the kit as `../../kit` and are copied into the skill's `examples/` folder before filming.

```
npm install                                  # here: the GIF encoder and playwright-core
cd ../../.agents/skills/stageshow && npm install && npm run dev   # the preview, on port 5173
```

Then, from this folder:

```
node film.mjs sync                           # copy examples/* into the skill
node film.mjs gate dictation-hero            # the skill's film.mjs --gate on the loop
node film.mjs gate dictation-settings --still
node film.mjs still dictation-settings --out ../../assets/dictation-settings.png   # a 2x PNG of the poster
node film.mjs gif dictation-hero --fps 20 --seconds 14 --out ../../assets/dictation-hero.gif
```

The GIF is recorded on the skill's virtual clock, one frame per step, so every run gives the same frames. Filming needs Google Chrome (`CHROME_PATH` points at another build).

Each demo has a `REHEARSAL.md` beside it: the gold example it started from, the beats, the gate's last line, every film line and why it stands, and what was not checked. Read `../../.agents/skills/stageshow/SKILL.md` before changing a demo.
