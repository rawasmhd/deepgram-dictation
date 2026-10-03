//! The app icon and the tray icon, drawn from code at any size.
//!
//! build.rs uses this file too, for the icon in dictation.exe, so it must
//! not call Windows.

use crate::gfx::{self, hex, lower_arc, mix, round_rect, Canvas, Rgb};

const TILE_FROM: Rgb = hex(0x1FA88C);
const TILE_TO: Rgb = hex(0x3B9EDB);
const WHITE: Rgb = [255.0; 3];

/// The tray icon's state dot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dot {
    None,
    Recording,
    Working,
    Problem,
}

/// The app icon: a microphone and sound bars on a teal-to-blue tile.
/// Below 24 px it has no bars, so the microphone stays clear.
pub fn app_icon(size: usize) -> Canvas {
    let mut c = Canvas::new(size, size);
    let n = size as f32;
    let tile = |x: f32, y: f32| mix(TILE_FROM, TILE_TO, (x + y) / (2.0 * n));
    if size >= 24 {
        // drawn on a 256 grid
        let k = n / 256.0;
        let s = |v: f32| v * k;
        c.paint_with([0.0, 0.0, n, n], |x, y| round_rect(x, y, s(8.0), s(8.0), s(248.0), s(248.0), s(56.0)), tile, 1.0);
        let all = [0.0, 0.0, n, n];
        c.paint(all, |x, y| round_rect(x, y, s(108.0), s(60.0), s(148.0), s(140.0), s(20.0)), WHITE, 1.0);
        let w = s(12.0);
        let mic = |x: f32, y: f32| {
            lower_arc(x, y, s(128.0), s(122.0), s(40.0), w)
                .min(gfx::line(x, y, s(88.0), s(116.0), s(88.0), s(122.0), w))
                .min(gfx::line(x, y, s(168.0), s(116.0), s(168.0), s(122.0), w))
                .min(gfx::line(x, y, s(128.0), s(162.0), s(128.0), s(184.0), w))
                .min(gfx::line(x, y, s(108.0), s(184.0), s(148.0), s(184.0), w))
        };
        c.paint(all, mic, WHITE, 1.0);
        let bars = |x: f32, y: f32| {
            gfx::line(x, y, s(68.0), s(96.0), s(68.0), s(152.0), w)
                .min(gfx::line(x, y, s(50.0), s(112.0), s(50.0), s(136.0), w))
                .min(gfx::line(x, y, s(188.0), s(96.0), s(188.0), s(152.0), w))
                .min(gfx::line(x, y, s(206.0), s(112.0), s(206.0), s(136.0), w))
        };
        c.paint(all, bars, WHITE, 0.72);
    } else {
        // drawn on a 16 grid
        let k = n / 16.0;
        let s = |v: f32| v * k;
        let all = [0.0, 0.0, n, n];
        c.paint_with(all, |x, y| round_rect(x, y, s(0.5), s(0.5), s(15.5), s(15.5), s(3.5)), tile, 1.0);
        c.paint(all, |x, y| round_rect(x, y, s(6.0), s(2.5), s(10.0), s(9.5), s(2.0)), WHITE, 1.0);
        let w = s(1.3);
        let mic = |x: f32, y: f32| {
            lower_arc(x, y, s(8.0), s(7.6), s(3.7), w).min(gfx::line(x, y, s(8.0), s(11.4), s(8.0), s(13.4), w))
        };
        c.paint(all, mic, WHITE, 1.0);
    }
    c
}

/// The tray icon: the app icon in one colour, as a solid tile with the
/// microphone and the bars cut out. White for a dark taskbar, near-black
/// for a light one. A dot in the corner shows the state.
pub fn tray_icon(size: usize, dark_taskbar: bool, dot: Dot) -> Canvas {
    let mut c = Canvas::new(size, size);
    let n = size as f32;
    let k = n / 16.0;
    let s = |v: f32| v * k;
    let all = [0.0, 0.0, n, n];
    let fg = if dark_taskbar { WHITE } else { hex(0x1A1A1A) };

    c.paint(all, |x, y| round_rect(x, y, s(1.0), s(1.0), s(15.0), s(15.0), s(3.5)), fg, 1.0);
    let w = s(1.3);
    c.erase(all, |x, y| {
        round_rect(x, y, s(6.6), s(3.4), s(9.4), s(8.6), s(1.4))
            .min(lower_arc(x, y, s(8.0), s(7.6), s(2.9), w))
            .min(gfx::line(x, y, s(8.0), s(10.5), s(8.0), s(12.4), w))
            .min(gfx::line(x, y, s(3.4), s(6.6), s(3.4), s(9.4), w))
            .min(gfx::line(x, y, s(12.6), s(6.6), s(12.6), s(9.4), w))
    });

    let colour = match (dot, dark_taskbar) {
        (Dot::None, _) => return c,
        (Dot::Recording, true) => hex(0xF05454),
        (Dot::Recording, false) => hex(0xD13438),
        (Dot::Working, true) => hex(0x38BDA0),
        (Dot::Working, false) => hex(0x0B7A68),
        (Dot::Problem, true) => hex(0xF5B942),
        (Dot::Problem, false) => hex(0xB86E00),
    };
    // a clear gap around the dot, so it reads against the tile
    c.erase(all, |x, y| gfx::circle(x, y, s(13.0), s(12.8), s(3.5)));
    c.paint(all, |x, y| gfx::circle(x, y, s(13.0), s(12.8), s(2.7)), colour, 1.0);
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(c: &Canvas, x: usize, y: usize) -> f32 {
        c.px[y * c.w + x][3]
    }

    #[test]
    fn app_icon_has_round_corners_and_a_solid_middle() {
        for size in [16, 32, 256] {
            let c = app_icon(size);
            assert_eq!(alpha(&c, 0, 0), 0.0, "{size}");
            assert_eq!(alpha(&c, size / 2, size / 2), 1.0, "{size}");
        }
    }

    #[test]
    fn tray_icon_cuts_out_the_microphone() {
        let c = tray_icon(32, true, Dot::None);
        assert_eq!(alpha(&c, 16, 10), 0.0); // inside the microphone
        assert_eq!(alpha(&c, 4, 4), 1.0); // the tile
    }

    #[test]
    fn tray_dot_is_drawn_in_the_corner() {
        let c = tray_icon(32, true, Dot::Recording);
        let p = c.px[26 * 32 + 26];
        assert_eq!(p[3], 1.0);
        assert!(p[0] > 200.0 && p[1] < 120.0); // red
    }
}
