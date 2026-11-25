# Rehearsal: dictation-tray

Gold: none for a native menu; the menu is cloned from assets/tray-menu.png (412 wide, rows 25 tall, labels 60px in, shortcuts 47px from the right, separators nearly full width), as craft.md asks for a real product's menu. It sits where the product puts it: above the tray, which runs off three edges. Every row is a real word, so the menu is whole.

Captured: wide (560 × 320) still at 2x. Not captured: 390.

Gate: `rehearsal failed: 1 never-intentional line(s) above` (`uneven gutters: div.dt-menu sits 46px from the left of div.ss-stage, 154px from the right and 48px from the bottom`)

Film:
- `uneven padding` (top 12 vs bottom 10) → the menu's padding is 8px top and bottom.
- `uneven gutters` → shots/dictation-tray.png → stands. The fresh-eye review sent back the first version (the menu centred in a 444-wide stage, 16px gutters, which passed this check) because a tray menu opens flush with the taskbar's top edge, its right edge over the icon it came from. That is where it sits now: the gap on the right is the rest of the tray (wifi, sound, battery, the clock), the gap below is the taskbar. The check reads the menu as a card on a plate; it is anchored UI (framing.md, rule 16).

Assumptions: the crop is the right end of a wide screen, so the taskbar's centred app tiles are out of frame.

Unresolved: none.

Not checked: the hover state of a row.
