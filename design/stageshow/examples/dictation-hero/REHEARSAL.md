# Rehearsal: dictation-hero

Gold: dark-hero (the window a step lighter than the page, one lit colour, filler rows in a sidebar) with hero-demo's corner bleed. Kept: the dark well, the window inset top-left and off the right and bottom, bars for everything the story is not about. Changed: Windows 11 chrome instead of macOS lights, no cursor (a keyboard story, told by keycaps), the product's own meter as the moving object.

Beats: 1 Alt+M, the meter rises · 2 the voice runs, the timer counts · 3 the first phrase is pasted while the voice goes on · 4 Alt+M, Transcribing · 5 the last phrase lands and the meter leaves (payoff, held 2.2s) · reset: Ctrl+Alt+Z, the app's own undo, takes the dictation back as one thing. Loop 13.8s.

Captured: wide (720 × 340) loop and states, poster still at 2x, GIF at 20 fps. Not captured: 390 (no compact design; the output is an image, and GitHub scales it).

Gate: `rehearsal passed (necessary, not sufficient: references/verify.md and the fresh eye in review.md still stand)`

Film:
- `jump: dd-meter__bar height/y` (many) → dictation-hero-gate.png → stands: overlay.rs draws a new level every frame and the bars step with it; a tween would read as a different product.
- `jump: dh-caret x/y at phrase2` → dictation-hero-gate.png (10.0s) → stands: a paste lands as one operation; the caret moves to its end. The span carries `data-snap`.
- `manual: same-box content change: dd-meter__bars, dd-meter__mark` → inspected on the contact sheet: the bars are the level history; the dot fades out as Transcribing fades in.
- `sampling gaps exceeded 27ms` → the real-time film on this machine; the GIF is recorded on the virtual clock, one frame per step, so it has no gaps.
- At 390: `shrunk, not laid out for the phone`, `small text` → stands: no compact design (see Captured).

Review (fresh eye, review.md):
- The sidebar's search box was an empty rounded rectangle → a magnifier glyph and a placeholder bar inside it.
- The keycaps sat over the sidebar's bars → moved to the empty band above the window's right half.
- "The meter appears whole in a single step" → judged from film.mjs at `--slow 4`, where the 280ms fade did not show. In a plain page the opacity runs 0 → 1 over 280ms (probed with playwright), and the GIF's own frames show it (shots/frames/024.png is mid-fade). Stands as a film artefact, not a scene fault.
- While the meter faded out its bars re-laid out under the Transcribing label → the meter now keeps drawing its last state while hidden (shots/frames/197.png).
- The dot and Transcribing were both half-visible in one frame → the dot and timer leave in 100ms, the label arrives 120ms later.
- The sparser pane beside dark-hero: accepted; a note has two sentences and air, and the meter fills the lower half.

Assumptions: the note is a generic notes app (no real product's identity); the voice is a deterministic function of the clock, not a recording.

Unresolved: none.

Not checked: live playback in the viewer at speed (the GIF was watched frame by frame on the contact sheet); reduced motion (the poster is the still).
