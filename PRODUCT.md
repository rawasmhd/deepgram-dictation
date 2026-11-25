# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

The shipping app is a native Windows program (`rust/`, Win32 and GDI, no web view). `web` here means the design prototypes are HTML pages. A chosen direction is then rebuilt in Rust by hand, so every visual decision must be drawable with plain fills, strokes, text and per-pixel alpha, without a browser.

## Users

One person at a Windows PC who prefers to talk instead of type. They press a hotkey, speak, and the text lands in whatever app has the cursor: an editor, a chat, a form. Confirmed audience: the owner (a developer) and anyone who installs the open-source `dictation.exe`. Inferred, not confirmed: most use is short bursts of dictation many times a day, often in a shared or noisy room.

## Product Purpose

Push-to-talk dictation for Windows that works in any application and shows almost nothing on screen. Press Alt+M, speak, press Alt+M again; the text is pasted at the cursor. Success is text that arrives correctly, fast, with the fewest possible things to look at or decide.

## Positioning

The app has no window of its own while you work. It is a hotkey, a small floating meter above the taskbar and a tray icon. Each phrase is pasted live while you speak, so all the text is already in place the moment you stop. One file, about 1 MB, no runtime, about 15 MB of memory.

## Operating Context

- The user is always inside another app. The meter must read in a glance over any background, light or dark, and must never take focus or a click.
- The meter sits just above the taskbar, centred. Windows may be in light or dark mode; the taskbar has its own light/dark setting, separate from apps.
- States the user sees, in order of frequency: Ready (tray only), Recording (meter with live level bars, red dot and timer), Paused (another window took focus; live paste waits), Transcribing (after stop, short), a Notice (information such as "Window changed - text copied", or an error such as "Microphone unavailable", "No speech detected", "Key rejected", "No connection to Deepgram", "Clipboard busy"), and Problem (tray dot).
- Settings window: paste a Deepgram API key (checked with Deepgram before save), show/hide the key, a link to console.deepgram.com, a switch "Start when I sign in to Windows", Save and Cancel, a status line for checking/errors. Opened at first run, from the tray, or with `--setup`.
- Custom words window: a text field with Add, a list of words with Remove, a count with a yellow warning near the limit and a red one over it, Save and Cancel. Saves to `words.txt`.
- Tray: an icon with a corner dot (none, red recording, teal working, amber problem) that adapts to a light or dark taskbar; a menu (name and state line, Start/Stop dictation Alt+M, Settings…, Custom words…, Start at login with a check, Open log file, Quit Ctrl+Alt+Q); balloon notices.
- Hotkeys: Alt+M start/stop, Ctrl+Alt+Z delete the last dictation, Ctrl+Alt+Q quit. Taps under 0.4 s are ignored.

## Capabilities and Constraints

- Everything is drawn by the app's own code: the meter is a layered window with per-pixel alpha at about 30 fps; the windows are owner-drawn GDI with Segoe UI; only the key field and the words field are standard edit controls. Any look must be reproducible this way: solid colours, gradients by hand, rounded rectangles, simple shapes, text. No blur, no web fonts without shipping them, no heavy animation.
- Both Windows light and dark mode are followed today and must stay supported. The tray icon follows the taskbar's own light/dark setting.
- The meter is 336 x 52 logical px today; its size is free to change but it must stay small, non-interactive and above the taskbar.
- Undecided: whether the new look ships one fixed theme or keeps following Windows light/dark. The prototypes show both so the owner can decide.
- Terminology used in the product and docs: dictation, meter, custom words, live paste, Transcribing, Paused, tray.

## Brand Commitments

- Name: "Deepgram Dictation" in window titles and the tray; repository `deepgram-dictation`. No logo beyond the drawn app icon (microphone with sound bars on a teal-to-blue tile) and the tray icon (same mark in one colour). The icon may change with the new look.
- Voice: plain, short, Simplified Technical English. UI copy says what happens ("Transcribing", "Paused", "Window changed - text copied").
- Redesign decided on 2026-11-11: anything goes. Windows 11 conventions may be left behind where the result is better. Three directions are prototyped; one is picked and then locked.

## Evidence on Hand

- Real screens of the current look: `assets/dictation-settings.png`, `assets/dictation-words.png`, `assets/dictation-paused.png`, `assets/dictation-tray.png`, `assets/dictation-hero.gif`.
- Current colours and sizes in `rust/src/setup.rs` (window palettes, light and dark), `rust/src/overlay.rs` (meter), `rust/src/art.rs` (icons).
- No testimonials, customers, benchmarks or pricing beyond Deepgram's own: do not invent any.

## Product Principles

1. Nothing to look at unless something changed. The meter and the tray carry state; there is no main window.
2. One glance tells the state. Recording, paused, working and a problem must be told apart by shape and colour, not by reading.
3. The text matters, not the app. The UI stays out of the way of the app the user is working in.
4. Drawable by hand. Every element can be drawn with GDI by the app's own code.
5. Short words, no surprises. Copy and controls do what they say.
