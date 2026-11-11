---
version: 1
slug: "design-prototypes-prompter-html"
primary_target: "design/prototypes/prompter.html"
related_targets: ["design/prototypes/tiles.html","design/prototypes/studio.html"]
---

# Surface brief: design/prototypes (meter, Settings, Custom words, tray)

Scope: three HTML prototype pages, one per visual direction, each showing the same four surfaces in every state, light and dark. Visitor mode: Operate (the owner compares them and picks one). Audience: the owner. Job: see each direction at real size and choose. Constraints: everything must be drawable with GDI and Windows-shipped fonts; the meter stays small and non-interactive; the state vocabulary from design/system/README.md holds in all three.

Decision round 2026-11-11, seed key 3cb6b81f, mode operate, assigned index 7. Grounded list, ordered by resonance: 1 broadcast studio ON AIR, 2 field radio push-to-talk, 3 Dictaphone microcassette recorder, 4 court transcript, 5 typewriter manuscript, 6 telephone switchboard, 7 teleprompter. The owner asked for three directions to compare in prototypes, so the decision page was replaced by the prototypes themselves. Built: the assigned direction (7, Prompter), the winning challenger (Metro typographic tiles, digital-design-canon-metro-typographic-tiles; wins on audience identification for Windows users, holds product clarity), and the pick (1, Studio). Competitive: terminal yellow wayfinding (holds clarity; one message per band kept as a rule for the meter). Declined: star atlas (kept: one fixed ramp for the level), vertical video feed (kept: white glyphs as the only interface colour in Prompter), jet-age ticket wallet (kept: the timer in a fixed field), Ikeda datamatics (kept: pixel-crisp level bars, no grey between on and off). Code-led: no image generation in this harness.

## Direction contract

### Prompter (assigned, candidate 7)

THESIS: the meter is a teleprompter cue line: black glass, one white line of large type, a marker that tells where you are. It refuses the rounded dark pill with coloured bars.
OWN-WORLD: pure black ground, white type in Segoe UI Semibold uppercase tracked 0.08em, a white cue triangle, level as white blocks on black, red and amber only for the recording and paused marks, square corners, 1 px grey lines, one theme in both Windows modes.
STORY: the user sees a prompter strip above the taskbar, reads the state in one word, and goes back to speaking.
FIRST VIEWPORT: the meter at 420 x 56 above a taskbar, the cue triangle at the left, white level blocks in the middle, the timer at 28 px tabular at the right; below it the same strip in Paused, Transcribing, info and error; then Settings and Custom words at 460 wide; then the tray.
FORM: teleprompter, position 7 of 7, seed 3cb6b81f.
FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance.

### Tiles (challenger, metro typographic tiles)

THESIS: the meter is a live tile: a flat teal rectangle with "recording" in giant lowercase cropped at the edge, and the state flips the tile. It refuses chrome of any kind.
OWN-WORLD: pure black or pure white ground, flat Metro teal #00aba9 as the one accent, Metro red, orange and cyan for the recording, paused and transcribing tiles, Segoe UI Light 40 px lowercase headings, 2 px borders on buttons, white fields with black text, no radius, no shadow.
STORY: the user sees a teal tile light up above the taskbar, the level squares fill with their voice, and the tile flips to "transcribing" when they stop.
FIRST VIEWPORT: the meter tile at 360 x 56, "recording" cropped at the right edge, 20 white level squares, the timer; the flipped tiles for Paused, Transcribing, info and error; Settings and Custom words with a giant lowercase heading cropped at the window edge; the tray with square state marks.
FORM: Metro typographic tiles, dealt challenger, seed 3cb6b81f.
FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance.

### Studio (pick, candidate 1)

THESIS: the meter is an instrument on a broadcast desk: a red ON AIR lamp, a VU needle and engraved numerals in a bezel. It refuses the flat dark pill.
OWN-WORLD: bakelite brown-black ground and panel with cream engraved labels in Bahnschrift uppercase tracked 0.12em, brass for actions, lamps in red, amber and green for recording, paused and transcribing, a VU arc with a red zone, rounded bezels with a soft drop shadow, a cream enamel version for Windows light mode.
STORY: the user sees the lamp light red, the needle swing with their voice, and the lamp turn green while the words arrive.
FIRST VIEWPORT: the meter bezel at 380 x 64 with the lamp at the left, the VU arc in the middle, the timer engraved at the right; the amber and green lamp states and the two notices; Settings and Custom words as panels with bakelite keys and a paddle switch; the tray with lamp dots.
FORM: broadcast studio ON AIR, position 1 of 7, seed 3cb6b81f.
FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance.

Unresolved: which direction ships; whether the chosen direction keeps following Windows light mode.
