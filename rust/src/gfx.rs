//! A small antialiased rasterizer: shapes as signed distance functions,
//! painted into an RGBA canvas. Used for the icons (also by build.rs, so
//! no Windows calls here), the setup window and the meter.

pub type Rgb = [f32; 3];

/// Premultiplied RGBA, colour channels 0-255, alpha 0-1.
#[derive(Clone)]
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 4]>,
}

impl Canvas {
    /// A transparent canvas.
    pub fn new(w: usize, h: usize) -> Self {
        Canvas { w, h, px: vec![[0.0; 4]; w * h] }
    }

    /// An opaque canvas filled with `bg`.
    pub fn filled(w: usize, h: usize, bg: Rgb) -> Self {
        Canvas { w, h, px: vec![[bg[0], bg[1], bg[2], 1.0]; w * h] }
    }

    /// Paint `colour` where the distance `d(x, y)` is negative, with a one
    /// pixel soft edge. `bbox` (x0, y0, x1, y1) limits the work.
    pub fn paint(&mut self, bbox: [f32; 4], d: impl Fn(f32, f32) -> f32, colour: Rgb, alpha: f32) {
        self.paint_with(bbox, d, |_, _| colour, alpha);
    }

    /// Like `paint`, with a colour per pixel (for gradients).
    pub fn paint_with(
        &mut self,
        bbox: [f32; 4],
        d: impl Fn(f32, f32) -> f32,
        colour: impl Fn(f32, f32) -> Rgb,
        alpha: f32,
    ) {
        self.each(bbox, d, |p, a, x, y| {
            let c = colour(x, y);
            let a = a * alpha;
            for i in 0..3 {
                p[i] = p[i] * (1.0 - a) + c[i] * a;
            }
            p[3] = p[3] * (1.0 - a) + a;
        });
    }

    /// Cut a shape out: make it transparent.
    pub fn erase(&mut self, bbox: [f32; 4], d: impl Fn(f32, f32) -> f32) {
        self.each(bbox, d, |p, a, _, _| {
            for v in p.iter_mut() {
                *v *= 1.0 - a;
            }
        });
    }

    fn each(&mut self, bbox: [f32; 4], d: impl Fn(f32, f32) -> f32, mut f: impl FnMut(&mut [f32; 4], f32, f32, f32)) {
        let x0 = (bbox[0] - 1.0).floor().max(0.0) as usize;
        let y0 = (bbox[1] - 1.0).floor().max(0.0) as usize;
        let x1 = ((bbox[2] + 1.0).ceil().max(0.0) as usize).min(self.w);
        let y1 = ((bbox[3] + 1.0).ceil().max(0.0) as usize).min(self.h);
        for y in y0..y1 {
            for x in x0..x1 {
                let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                let a = (0.5 - d(cx, cy)).clamp(0.0, 1.0);
                if a > 0.0 {
                    f(&mut self.px[y * self.w + x], a, cx, cy);
                }
            }
        }
    }

    /// Draw `other` on top, with its top left corner at (x, y).
    pub fn draw(&mut self, other: &Canvas, x: usize, y: usize) {
        for oy in 0..other.h {
            for ox in 0..other.w {
                let (tx, ty) = (x + ox, y + oy);
                if tx >= self.w || ty >= self.h {
                    continue;
                }
                let s = other.px[oy * other.w + ox];
                let p = &mut self.px[ty * self.w + tx];
                for i in 0..4 {
                    p[i] = s[i] + p[i] * (1.0 - s[3]);
                }
            }
        }
    }

    /// Premultiplied BGRA, for layered windows and GDI.
    pub fn bgra_premultiplied(&self) -> Vec<u32> {
        self.px
            .iter()
            .map(|p| byte(p[3] * 255.0) << 24 | byte(p[0]) << 16 | byte(p[1]) << 8 | byte(p[2]))
            .collect()
    }

    /// Straight (not premultiplied) BGRA, for icons.
    pub fn bgra_straight(&self) -> Vec<u32> {
        self.px
            .iter()
            .map(|p| {
                let a = p[3];
                let un = |v: f32| if a > 0.0 { v / a } else { 0.0 };
                byte(a * 255.0) << 24 | byte(un(p[0])) << 16 | byte(un(p[1])) << 8 | byte(un(p[2]))
            })
            .collect()
    }
}

pub fn byte(v: f32) -> u32 {
    (v + 0.5).clamp(0.0, 255.0) as u32
}

pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// `0xRRGGBB` as a colour.
pub const fn hex(v: u32) -> Rgb {
    [((v >> 16) & 255) as f32, ((v >> 8) & 255) as f32, (v & 255) as f32]
}

// -- distance functions (negative inside) ---------------------------------------

/// A rectangle from (x0, y0) to (x1, y1) with corner radius `r`.
pub fn round_rect(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> f32 {
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (hw, hh) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    let r = r.min(hw).min(hh);
    let qx = (x - cx).abs() - (hw - r);
    let qy = (y - cy).abs() - (hh - r);
    let (ox, oy) = (qx.max(0.0), qy.max(0.0));
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

pub fn circle(x: f32, y: f32, cx: f32, cy: f32, r: f32) -> f32 {
    ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - r
}

/// A line from a to b with round ends, `w` wide.
pub fn line(x: f32, y: f32, ax: f32, ay: f32, bx: f32, by: f32, w: f32) -> f32 {
    let (px, py, dx, dy) = (x - ax, y - ay, bx - ax, by - ay);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { ((px * dx + py * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    ((px - dx * t).powi(2) + (py - dy * t).powi(2)).sqrt() - w / 2.0
}

/// The lower half of a circle outline, `w` wide, with round ends.
pub fn lower_arc(x: f32, y: f32, cx: f32, cy: f32, r: f32, w: f32) -> f32 {
    if y >= cy {
        (circle(x, y, cx, cy, 0.0) - r).abs() - w / 2.0
    } else {
        circle(x, y, cx - r, cy, 0.0).min(circle(x, y, cx + r, cy, 0.0)) - w / 2.0
    }
}

/// The outline of a shape, `w` wide, centred on its edge.
pub fn outline(d: f32, w: f32) -> f32 {
    d.abs() - w / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filled_shape_is_opaque_inside_and_clear_outside() {
        let mut c = Canvas::new(10, 10);
        c.paint([2.0, 2.0, 8.0, 8.0], |x, y| round_rect(x, y, 2.0, 2.0, 8.0, 8.0, 1.0), [255.0; 3], 1.0);
        assert_eq!(c.px[5 * 10 + 5][3], 1.0);
        assert_eq!(c.px[0][3], 0.0);
    }

    #[test]
    fn erase_cuts_a_hole() {
        let mut c = Canvas::filled(10, 10, [0.0; 3]);
        c.erase([0.0, 0.0, 10.0, 10.0], |x, y| circle(x, y, 5.0, 5.0, 3.0));
        assert_eq!(c.px[5 * 10 + 5][3], 0.0);
        assert_eq!(c.px[0][3], 1.0);
    }

    #[test]
    fn straight_alpha_undoes_the_premultiply() {
        let mut c = Canvas::new(1, 1);
        c.px[0] = [100.0, 50.0, 0.0, 0.5];
        assert_eq!(c.bgra_straight()[0], 128 << 24 | 200 << 16 | 100 << 8);
    }
}
