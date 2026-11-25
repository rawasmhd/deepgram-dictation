# Deepgram Dictation design system

The rules for everything the app draws: the floating meter, the Settings and Custom words windows, the tray icon, the tray menu and the balloons. Issue #71.

The system has two layers:

1. **The contract.** Token names, sizes, spacing, motion, the state vocabulary, the component list and the rules below. It does not change between directions.
2. **The direction.** The values behind the token names: colour, type, shape. Three candidates exist (`tokens.css`): **Prompter**, **Tiles**, **Studio**. One is picked from the prototypes in `design/prototypes/`. Then the other two are deleted and `DESIGN.md` records the survivor.

Everything here must be drawable by the app's own code with GDI: solid fills, gradients made by hand, 1 px strokes, rounded rectangles, text, and per-pixel alpha on the meter. No blur. No web fonts unless the app ships them. Fonts are limited to what Windows 10 and 11 install: Segoe UI (Variable), Bahnschrift, Cascadia Mono, Consolas.

## Principles

1. **Nothing to look at unless something changed.** There is no main window. The meter and the tray carry the state.
2. **One glance tells the state.** Recording, paused, transcribing and a problem differ by shape and colour, not only by a word.
3. **The text matters, not the app.** The meter never takes focus or a click, and never hides the taskbar.
4. **Drawable by hand.** If a GDI drawing routine cannot make it, it is not in the system.
5. **Short words, no surprises.** Controls say what they do. Copy is plain English.

## Surfaces

| Surface | Size (logical px) | Notes |
|---|---|---|
| Meter | direction sets `--dd-meter-w` x `--dd-meter-h` | Centred above the taskbar, 12 px gap. Layered window, per-pixel alpha, ~30 fps. Never focusable. |
| Settings window | 460 wide, content sets height | Title row, heading, API key field with Show key, help line with link, Start-at-login switch, status line, footer with Save and Cancel. |
| Custom words window | 460 wide | Title row, heading, field with Add, list with Remove, count with warnings, status line, footer. |
| Tray icon | 16 px grid, drawn at the taskbar's scale | One-colour mark on the taskbar; a corner dot for the state. Follows the taskbar's own light/dark setting, not the apps setting. |
| Tray menu | 32 px rows | Name and state line (disabled), Start/Stop dictation, Settings…, Custom words…, Start at login (check), Open log file, Quit. Shortcuts right-aligned. |
| Balloon | Windows draws it | Only the icon and the words are ours. |
| App icon | 16 to 256 px | Drawn from code in `art.rs`. The direction sets the tile and the mark. |

## States

The meter and the tray share one vocabulary. Each state has a colour token, a shape and a word.

| State | Token | Shape in the meter | Word | Tray dot |
|---|---|---|---|---|
| Ready | none | meter hidden | none | none |
| Recording | `--dd-rec` | the recording mark (dot, lamp or tile) and live level | timer `m:ss` | red |
| Paused | `--dd-pause` | the pause mark; level keeps moving | "Paused" | red (still recording) |
| Transcribing | `--dd-work` | the working mark; level fades | "Transcribing" | teal / work |
| Notice, info | `--dd-info` | the info mark | the message | none |
| Notice, error | `--dd-problem` | the error mark | the message | amber |
| Problem (tray only) | `--dd-problem` | | balloon | amber |

Rules:

- A state word is always next to its mark. Never a mark alone, never a word alone, except the timer while recording.
- The timer uses `--dd-font-num` with tabular figures.
- Messages are short and name the problem and the recovery when there is one: "Microphone unavailable", "No speech detected", "Key rejected - check your API key", "No connection to Deepgram", "Clipboard busy", "Window changed - text copied".
- A notice stays for 2.4 s (error 3.2 s), then the meter hides.

## Level display

The level history is 40 samples at 30 fps, newest at the right. The direction decides the drawing (bars, blocks, a needle) but these hold:

- `--dd-level-0` for silence, `--dd-level-1` for speech, `--dd-level-2` for loud. Loud is the top 15 % of the range.
- The level keeps moving while Paused, so the user sees the microphone is still open.
- While Transcribing, the level stops and the working mark takes its place within `--dd-base`.

## Tokens

The names and the fixed values are in [`tokens.css`](tokens.css). In short:

- **Spacing**: a 4 px grid, `--dd-space-1` (4) to `--dd-space-6` (32). Window padding is 24. Groups are 8 or 12 apart, sections 24 apart. More space above a label than below it.
- **Type**: 12, 13, 14, 16, 20, 28, 40 px. One family per direction, set in `--dd-font`, `--dd-font-display`, `--dd-font-num`. Body is 14 px. Headings use `--dd-case-display` and `--dd-track-display`.
- **Sizes**: fields and buttons 36 px high, rows 32 px, buttons 96 px wide in the footer.
- **Shape**: `--dd-radius-sm/md/lg/meter` and `--dd-stroke` are set by the direction.
- **Motion**: 120 ms for hover and press, 200 ms for a state change, 280 ms for the meter to appear or hide. One ease, `--dd-ease`. Nothing moves that is not a state change, except the level and the breathing recording mark.

## Components

Each component lists its parts and its states. Every interactive component has default, hover, pressed, focused and disabled. Keyboard focus is always visible.

**Meter.** A panel (`--dd-panel`, `--dd-panel-edge`, `--dd-shadow`, `--dd-radius-meter`) with three zones: the mark at the left, the level in the middle, the word at the right. The direction may merge zones (Tiles puts the word in the tile) but keeps the reading order mark, level, word.

**Mark.** The small shape that carries a state: a dot, a lamp, a square, a cue triangle. Recording breathes: opacity 0.1 to 0.28 over 1.26 s on a halo, or an equivalent the direction defines.

**Window frame.** Title row 32 px with the app icon at 16 px and the window title at 13 px. Below it 24 px padding. Heading 28 px with a one-line subtitle in `--dd-ink-2`. Content. Footer: a strip in a second neutral with Cancel and Save at the right, Save is the primary button and the default (Enter). Esc cancels.

**Text field.** 36 px high, `--dd-field`, `--dd-field-edge` at `--dd-stroke`, focus shows `--dd-field-focus`. Placeholder in `--dd-ink-3`. The key field masks with bullets and has Show key at its right end.

**Button.** 36 px high, 96 px wide in a footer, otherwise content plus 2 x 16 px. Secondary: `--dd-panel` fill with `--dd-line` edge. Primary: `--dd-accent` fill with `--dd-on-accent` text. Disabled: `--dd-ink-3` text, no hover.

**Switch.** A two-position control for Start when I sign in to Windows. On uses `--dd-accent`. The direction sets the shape (pill, square, paddle). The label is at the left, the switch at the right of the row.

**Link.** `--dd-accent`, underline on hover, used only for console.deepgram.com.

**List.** Rows of `--dd-row`. Selected row uses `--dd-accent` at low alpha or a full flood (Tiles). Delete removes the selected row.

**Status line.** One line under the content for "Checking the key…", "Saved", or an error in `--dd-problem`. Count warnings: near the limit in `--dd-pause`, over it in `--dd-problem`.

**Tray icon.** The mark in one colour: white on a dark taskbar, near-black on a light one. The state dot at the lower right with a clear gap around it.

**Tray menu.** `--dd-panel`, rows 32 px, 12 px side padding, a 1 px `--dd-line` separator between groups. Hover/selection as the direction defines. Shortcuts in `--dd-ink-2`.

**App icon.** A tile with the mark. The direction sets the tile's colour and corner radius. Below 24 px the mark loses its bars.

## Light and dark

The app follows the Windows apps mode. A direction answers it in one of two ways and says which in `--dd-follows-windows`:

- **yes**: it has a dark and a light token set (Tiles, Studio).
- **no**: one set for both modes (Prompter is black always).

The tray icon follows the taskbar's own setting in every direction.

## Accessibility

- Text contrast 4.5:1 or better against its surface, 3:1 for the 28 px heading and larger.
- State is never colour alone: each state has its own shape and word.
- Every control works with the keyboard: Tab order top to bottom, Enter saves, Esc cancels, Delete removes a word.
- The breathing mark and the level stop when Windows has animations off (`SPI_GETCLIENTAREAANIMATION`).

## The three directions

| | Prompter | Tiles | Studio |
|---|---|---|---|
| World | a teleprompter: black glass, large white type, a cue marker | Windows' own Metro: flat colour tiles, giant lowercase Segoe, no chrome | a broadcast studio: bakelite panel, engraved labels, ON AIR lamp, VU needle |
| Ground | black, always | black or white | bakelite brown-black or cream enamel |
| Type | Segoe UI, semibold, uppercase, tracked | Segoe UI Light, lowercase, large | Bahnschrift, uppercase, tracked |
| Shape | square corners, 1 px lines | square corners, 2 px borders, no shadows | rounded bezels, soft shadows |
| Live colour | white | Metro teal | brass |
| Meter | a black bar with a cue triangle, white level blocks, a large timer | a teal tile, "recording" cropped at the edge, white level squares | a bezel with a red lamp, a VU needle, engraved numerals |
| Follows Windows mode | no | yes | yes |

## How to lock a direction

1. Pick a prototype.
2. Delete the two other blocks from `tokens.css` and the two other prototype pages.
3. Write `DESIGN.md` at the repository root from the surviving values (the Impeccable documenter does this).
4. Open one issue per surface for the Rust work, each linking #71, in this order: meter, tray icon and menu, Settings window, Custom words window, app icon.
