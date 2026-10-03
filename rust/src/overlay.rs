//! The floating meter. A layered window with per-pixel alpha, so the
//! rounded ends and the shadow are smooth.
//! It never takes focus and never intercepts a click.

use std::{collections::VecDeque, mem, ptr, time::Instant};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::gfx::{self, byte, mix, Rgb};

// Layout in logical pixels (96 dpi). The window is larger than the panel
// by SHADOW on each side, for the drop shadow.
const W: f32 = 336.0;
const H: f32 = 52.0;
const RADIUS: f32 = H / 2.0;
const SHADOW: f32 = 16.0;
const SHADOW_DROP: f32 = 4.0; // the shadow sits this much lower
const SHADOW_BLUR: f32 = 11.0;
const SHADOW_ALPHA: f32 = 0.30;
const BARS: usize = 40;
const PITCH: f32 = 5.0;
const BAR_W: f32 = 3.0;
const MAX_BAR: f32 = 15.0;
const BARS_X0: f32 = 40.0;
const PAD: f32 = 18.0;
const ICON_X: f32 = 26.0; // centre of the dot or icon on the left
const TEXT_X: f32 = 40.0; // a message after the icon
const MARGIN: f32 = 12.0 - SHADOW; // the panel's gap above the taskbar
const FONT_PX: f32 = 13.0;
const WORK_LABEL: &str = "Transcribing";

pub const TIMER_FRAME: usize = 1;
const FRAME_MS: u32 = 33; // ~30 fps

const PANEL_BG: Rgb = [23.0, 24.0, 29.0];
const PANEL_EDGE: Rgb = [40.0, 42.0, 49.0];
const TEXT_BRIGHT: Rgb = [231.0, 233.0, 238.0];
const BAR_IDLE: Rgb = [58.0, 61.0, 71.0];
const BAR_LOW: Rgb = [56.0, 189.0, 160.0];
const BAR_HIGH: Rgb = [125.0, 211.0, 252.0];
const REC_RED: Rgb = [240.0, 84.0, 84.0];
const ERR_RED: Rgb = [255.0, 107.0, 107.0];
const ERR_TEXT: Rgb = [255.0, 138.0, 138.0];
const OK_TEAL: Rgb = [56.0, 189.0, 160.0];
const PAUSE_AMBER: Rgb = [245.0, 185.0, 66.0];

pub enum State {
    Hidden,
    Recording,
    Working,
    Notice { text: String, error: bool },
}

pub struct Overlay {
    hwnd: HWND,
    state: State,
    started: Instant,
    frame: u32,
    history: VecDeque<f32>,
    paused: bool,
    gfx: Option<Gfx>,
    pos: POINT,
}

/// Device resources for one DPI scale.
struct Gfx {
    scale: f32,
    w: i32,
    h: i32,
    dc: HDC,
    bmp: HBITMAP,
    old_bmp: HGDIOBJ,
    font: HFONT,
    bold: HFONT,
    old_font: HGDIOBJ,
    bits: *mut u32,
    base_rgb: Vec<Rgb>,   // the empty panel and its shadow
    base_alpha: Vec<f32>, // their coverage, 0 outside
    canvas: Vec<Rgb>,
    work_start: usize, // first bar to the right of the "Transcribing" label
}

pub fn create_window(wndproc: WNDPROC) -> HWND {
    let class = wide("DictationMeterRs");
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: wndproc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            class.as_ptr(),
            class.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        )
    }
}

impl Overlay {
    pub fn new(hwnd: HWND) -> Self {
        Overlay {
            hwnd,
            state: State::Hidden,
            started: Instant::now(),
            frame: 0,
            history: VecDeque::from(vec![0.0; BARS]),
            paused: false,
            gfx: None,
            pos: POINT { x: 0, y: 0 },
        }
    }

    /// Live paste is waiting for the focus to come back.
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn is_working(&self) -> bool {
        matches!(self.state, State::Working)
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn set_state(&mut self, state: State) {
        self.frame = 0;
        self.started = Instant::now();
        if matches!(state, State::Recording) {
            self.history = VecDeque::from(vec![0.0; BARS]);
            self.paused = false;
        }
        let hidden = matches!(state, State::Hidden);
        self.state = state;
        if hidden {
            self.hide();
        } else {
            self.show();
        }
    }

    pub fn tick(&mut self, mic_level: f32) {
        self.frame += 1;
        match self.state {
            State::Recording => {
                self.history.pop_front();
                self.history.push_back(mic_level);
            }
            State::Notice { .. } if self.started.elapsed().as_secs_f32() > 3.0 => {
                self.set_state(State::Hidden);
                return;
            }
            State::Hidden => return,
            _ => {}
        }
        self.render();
    }

    // -- window plumbing ----------------------------------------------------

    fn show(&mut self) {
        unsafe {
            // the monitor under the cursor, minus its taskbar
            let mut cursor = POINT { x: 0, y: 0 };
            GetCursorPos(&mut cursor);
            let mon = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
            let mut info: MONITORINFO = mem::zeroed();
            info.cbSize = mem::size_of::<MONITORINFO>() as u32;
            GetMonitorInfoW(mon, &mut info);
            let work = info.rcWork;

            let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
            GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
            let scale = dpi_x as f32 / 96.0;
            if self.gfx.as_ref().map_or(true, |g| g.scale != scale) {
                self.gfx = Some(Gfx::new(scale));
            }
            let g = self.gfx.as_ref().unwrap();
            self.pos = POINT {
                x: work.left + (work.right - work.left - g.w) / 2,
                y: work.bottom - g.h - (MARGIN * scale).round() as i32,
            };

            self.render();
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            SetTimer(self.hwnd, TIMER_FRAME, FRAME_MS, None);
        }
    }

    fn hide(&mut self) {
        unsafe {
            KillTimer(self.hwnd, TIMER_FRAME);
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    // -- drawing ------------------------------------------------------------

    fn render(&mut self) {
        let Some(g) = self.gfx.as_mut() else { return };
        let cy = H / 2.0;
        g.canvas.copy_from_slice(&g.base_rgb);

        let mut texts: Vec<(String, f32, bool, bool, Rgb)> = Vec::new(); // text, x, right-aligned, bold, colour

        match &self.state {
            State::Hidden => return,
            State::Recording => {
                for (i, &lvl) in self.history.iter().enumerate() {
                    let h = (lvl * MAX_BAR).max(BAR_W / 2.0);
                    let colour = if lvl < 0.02 { BAR_IDLE } else { mix(BAR_LOW, BAR_HIGH, lvl) };
                    g.bar(i, h, colour);
                }
                if self.paused {
                    let w = 2.4;
                    g.shape(
                        [ICON_X - 9.0, cy - 9.0, ICON_X + 9.0, cy + 9.0], 
                        |x, y| {
                            gfx::line(x, y, ICON_X - 3.0, cy - 4.0, ICON_X - 3.0, cy + 4.0, w)
                                .min(gfx::line(x, y, ICON_X + 3.0, cy - 4.0, ICON_X + 3.0, cy + 4.0, w))
                        },
                        PAUSE_AMBER,
                        1.0,
                    );
                    texts.push(("Paused".to_string(), W - PAD, true, true, PAUSE_AMBER));
                } else {
                    // a solid dot with a slowly pulsing halo
                    let p = 0.5 + 0.5 * (self.frame as f32 / 6.0).sin();
                    g.shape([ICON_X - 9.0, cy - 9.0, ICON_X + 9.0, cy + 9.0], |x, y| gfx::circle(x, y, ICON_X, cy, 8.0), REC_RED, 0.10 + 0.18 * p);
                    g.shape([ICON_X - 9.0, cy - 9.0, ICON_X + 9.0, cy + 9.0], |x, y| gfx::circle(x, y, ICON_X, cy, 4.0), REC_RED, 1.0);
                    let secs = self.started.elapsed().as_secs();
                    texts.push((format!("{}:{:02}", secs / 60, secs % 60), W - PAD, true, true, TEXT_BRIGHT));
                }
            }
            State::Working => {
                let vis = BARS - g.work_start;
                let head = (self.frame as f32 * 1.1) % (vis as f32 + 10.0);
                for j in 0..vis {
                    let glow = (1.0 - (j as f32 - head).abs() / 5.0).max(0.0);
                    g.bar(g.work_start + j, BAR_W / 2.0 + glow * 7.0, mix(BAR_IDLE, BAR_HIGH, glow));
                }
                texts.push((WORK_LABEL.to_string(), PAD, false, true, TEXT_BRIGHT));
            }
            State::Notice { text, error } => {
                let w = 1.6;
                let ring = |x: f32, y: f32| gfx::outline(gfx::circle(x, y, ICON_X, cy, 6.5), w);
                if *error {
                    g.shape(
                        [ICON_X - 9.0, cy - 9.0, ICON_X + 9.0, cy + 9.0], 
                        |x, y| {
                            ring(x, y)
                                .min(gfx::line(x, y, ICON_X, cy - 3.2, ICON_X, cy + 0.6, w))
                                .min(gfx::circle(x, y, ICON_X, cy + 3.3, 1.0))
                        },
                        ERR_RED,
                        1.0,
                    );
                } else {
                    g.shape(
                        [ICON_X - 9.0, cy - 9.0, ICON_X + 9.0, cy + 9.0], 
                        |x, y| {
                            ring(x, y)
                                .min(gfx::line(x, y, ICON_X - 2.8, cy + 0.2, ICON_X - 0.8, cy + 2.2, w))
                                .min(gfx::line(x, y, ICON_X - 0.8, cy + 2.2, ICON_X + 2.8, cy - 1.8, w))
                        },
                        OK_TEAL,
                        1.0,
                    );
                }
                let text = g.fit(text, W - PAD - TEXT_X);
                texts.push((text, TEXT_X, false, false, if *error { ERR_TEXT } else { TEXT_BRIGHT }));
            }
        }

        g.flush_canvas();
        for (text, x, right, bold, colour) in &texts {
            g.text(text, *x, *right, *bold, *colour);
        }
        g.finish_alpha();

        unsafe {
            let size = SIZE { cx: g.w, cy: g.h };
            let src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(
                self.hwnd,
                ptr::null_mut(),
                &self.pos,
                &size,
                g.dc,
                &src,
                0,
                &blend,
                ULW_ALPHA,
            );
        }
    }
}

impl Gfx {
    fn new(scale: f32) -> Self {
        let w = ((W + 2.0 * SHADOW) * scale).round() as i32;
        let h = ((H + 2.0 * SHADOW) * scale).round() as i32;
        let n = (w * h) as usize;
        unsafe {
            let screen = GetDC(ptr::null_mut());
            let dc = CreateCompatibleDC(screen);
            ReleaseDC(ptr::null_mut(), screen);

            let mut bmi: BITMAPINFO = mem::zeroed();
            bmi.bmiHeader.biSize = mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = w;
            bmi.bmiHeader.biHeight = -h; // top-down rows
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB as _;
            let mut bits: *mut core::ffi::c_void = ptr::null_mut();
            let bmp = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
            let old_bmp = SelectObject(dc, bmp);

            let face = wide("Segoe UI");
            let font = |weight: u32| {
                CreateFontW(
                    -(FONT_PX * scale).round() as i32,
                    0,
                    0,
                    0,
                    weight as _,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as _,
                    OUT_DEFAULT_PRECIS as _,
                    CLIP_DEFAULT_PRECIS as _,
                    CLEARTYPE_QUALITY as _,
                    0,
                    face.as_ptr(),
                )
            };
            let (font, bold) = (font(FW_NORMAL), font(FW_SEMIBOLD));
            let old_font = SelectObject(dc, font);
            SetBkMode(dc, TRANSPARENT as _);

            let mut g = Gfx {
                scale,
                w,
                h,
                dc,
                bmp,
                old_bmp,
                font,
                bold,
                old_font,
                bits: bits as *mut u32,
                base_rgb: vec![[0.0; 3]; n],
                base_alpha: vec![0.0; n],
                canvas: vec![[0.0; 3]; n],
                work_start: BARS,
            };
            g.build_panel();

            let label_right = PAD + g.measure(WORK_LABEL, true).cx as f32 / scale;
            g.work_start = (0..BARS)
                .find(|&i| BARS_X0 + i as f32 * PITCH >= label_right + 14.0)
                .unwrap_or(BARS);
            g
        }
    }

    /// Logical panel coordinates to window pixels.
    fn px(&self, v: f32) -> f32 {
        (v + SHADOW) * self.scale
    }

    /// The empty rounded panel with a 1px edge and a soft shadow below,
    /// antialiased. The colours are straight (not premultiplied).
    fn build_panel(&mut self) {
        let s = self.scale;
        let (x0, y0, x1, y1) = (self.px(0.0), self.px(0.0), self.px(W), self.px(H));
        let r = RADIUS * s;
        let edge = s.max(1.0);
        let drop = SHADOW_DROP * s;
        let blur = SHADOW_BLUR * s;
        for y in 0..self.h {
            for x in 0..self.w {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let d = gfx::round_rect(fx, fy, x0, y0, x1, y1, r);
                let panel = (0.5 - d).clamp(0.0, 1.0);
                let inner = (0.5 - (d + edge)).clamp(0.0, 1.0);

                let ds = gfx::round_rect(fx, fy, x0, y0 + drop, x1, y1 + drop, r);
                let t = (ds / blur).clamp(0.0, 1.0);
                let shadow = SHADOW_ALPHA * (1.0 - t * t * (3.0 - 2.0 * t));

                let alpha = panel + shadow * (1.0 - panel);
                let i = (y * self.w + x) as usize;
                self.base_alpha[i] = alpha;
                // the shadow is black, so it only darkens the edge pixels
                let colour = mix(PANEL_EDGE, PANEL_BG, inner);
                let k = if alpha > 0.0 { panel / alpha } else { 0.0 };
                self.base_rgb[i] = [colour[0] * k, colour[1] * k, colour[2] * k];
            }
        }
    }

    /// Bar `i`, `half` logical pixels above and below the centre line,
    /// with round ends.
    fn bar(&mut self, i: usize, half: f32, colour: Rgb) {
        let x = BARS_X0 + i as f32 * PITCH + BAR_W / 2.0;
        let r = BAR_W / 2.0;
        let cy = H / 2.0;
        let (top, bottom) = (cy - (half - r).max(0.0), cy + (half - r).max(0.0));
        self.shape([x - r, top - r, x + r, bottom + r], |px, py| gfx::line(px, py, x, top, x, bottom, BAR_W), colour, 1.0);
    }

    /// Paint a shape, given as a distance function in logical panel
    /// coordinates. `bbox` (x0, y0, x1, y1) limits the work.
    fn shape(&mut self, bbox: [f32; 4], d: impl Fn(f32, f32) -> f32, colour: Rgb, alpha: f32) {
        let s = self.scale;
        let [x0, y0, x1, y1] = bbox.map(|v| self.px(v));
        let (px0, px1) = ((x0 - 1.0).max(0.0) as i32, ((x1 + 1.0) as i32).min(self.w));
        let (py0, py1) = ((y0 - 1.0).max(0.0) as i32, ((y1 + 1.0) as i32).min(self.h));
        for y in py0..py1 {
            let ly = (y as f32 + 0.5) / s - SHADOW;
            for x in px0..px1 {
                let lx = (x as f32 + 0.5) / s - SHADOW;
                let a = (0.5 - d(lx, ly) * s).clamp(0.0, 1.0) * alpha;
                if a > 0.0 {
                    let i = (y * self.w + x) as usize;
                    self.canvas[i] = mix(self.canvas[i], colour, a);
                }
            }
        }
    }

    /// Copy the canvas into the bitmap, opaque, so GDI can draw text on it.
    fn flush_canvas(&mut self) {
        let px = unsafe { std::slice::from_raw_parts_mut(self.bits, self.canvas.len()) };
        for (dst, c) in px.iter_mut().zip(&self.canvas) {
            *dst = (byte(c[0]) << 16) | (byte(c[1]) << 8) | byte(c[2]);
        }
    }

    /// Text at logical `x`, vertically centred. GDI writes the colour
    /// channels only; `finish_alpha` adds the alpha afterwards.
    fn text(&self, text: &str, x: f32, right_aligned: bool, bold: bool, colour: Rgb) {
        let size = self.measure(text, bold);
        let mut left = self.px(x).round() as i32;
        if right_aligned {
            left -= size.cx;
        }
        let top = self.px(H / 2.0).round() as i32 - size.cy / 2;
        let t = wide_no_nul(text);
        unsafe {
            SetTextColor(self.dc, byte(colour[2]) << 16 | byte(colour[1]) << 8 | byte(colour[0]));
            TextOutW(self.dc, left, top, t.as_ptr(), t.len() as i32);
        }
    }

    /// Selects the font, and measures `text` in it.
    fn measure(&self, text: &str, bold: bool) -> SIZE {
        let t = wide_no_nul(text);
        let mut size = SIZE { cx: 0, cy: 0 };
        unsafe {
            SelectObject(self.dc, if bold { self.bold } else { self.font });
            GetTextExtentPoint32W(self.dc, t.as_ptr(), t.len() as i32, &mut size);
        }
        size
    }

    /// `text`, shortened with "…" to fit in `width` logical pixels.
    fn fit(&self, text: &str, width: f32) -> String {
        let max = (width * self.scale) as i32;
        if self.measure(text, false).cx <= max {
            return text.to_string();
        }
        let mut chars: Vec<char> = text.chars().collect();
        while chars.pop().is_some() {
            let t = chars.iter().collect::<String>().trim_end().to_string() + "…";
            if self.measure(&t, false).cx <= max {
                return t;
            }
        }
        String::new()
    }

    /// Apply the panel shape as premultiplied alpha.
    fn finish_alpha(&mut self) {
        unsafe { GdiFlush() };
        let px = unsafe { std::slice::from_raw_parts_mut(self.bits, self.base_alpha.len()) };
        for (p, &a) in px.iter_mut().zip(&self.base_alpha) {
            let ch = |shift: u32| (((*p >> shift) & 255) as f32 * a + 0.5) as u32;
            *p = (byte(a * 255.0) << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0);
        }
    }
}

impl Drop for Gfx {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old_font);
            SelectObject(self.dc, self.old_bmp);
            DeleteObject(self.font);
            DeleteObject(self.bold);
            DeleteObject(self.bmp);
            DeleteDC(self.dc);
        }
    }
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn wide_no_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
