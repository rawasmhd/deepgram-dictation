//! The floating meter. A layered window with per-pixel alpha, so the
//! rounded corners are smooth (dictate.py punches out a colour key instead).
//! It never takes focus and never intercepts a click.

use std::{collections::VecDeque, mem, ptr, time::Instant};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

// Layout in logical pixels (96 dpi), the same as dictate.py.
const W: f32 = 336.0;
const H: f32 = 68.0;
const RADIUS: f32 = 16.0;
const BARS: usize = 38;
const PITCH: f32 = 6.0;
const BAR_W: f32 = 3.0;
const MAX_BAR: f32 = 21.0;
const BARS_X0: f32 = 42.0;
const MARGIN: f32 = 12.0; // gap above the taskbar
const FONT_PT: f32 = 10.0;
const WORK_LABEL: &str = "Transcribing";

pub const TIMER_FRAME: usize = 1;
const FRAME_MS: u32 = 33; // ~30 fps

type Rgb = [f32; 3];
const PANEL_BG: Rgb = [23.0, 24.0, 29.0];
const PANEL_EDGE: Rgb = [49.0, 51.0, 60.0];
const TEXT_DIM: Rgb = [139.0, 143.0, 156.0];
const TEXT_BRIGHT: Rgb = [231.0, 233.0, 238.0];
const BAR_IDLE: Rgb = [58.0, 61.0, 71.0];
const BAR_LOW: Rgb = [56.0, 189.0, 160.0];
const BAR_HIGH: Rgb = [125.0, 211.0, 252.0];
const DOT_DIM: Rgb = [90.0, 40.0, 46.0];
const REC_RED: Rgb = [240.0, 84.0, 84.0];
const ERR_RED: Rgb = [255.0, 107.0, 107.0];
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
    old_font: HGDIOBJ,
    bits: *mut u32,
    base_rgb: Vec<Rgb>,  // the empty panel
    base_alpha: Vec<f32>, // panel coverage, 0 outside the rounded rect
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
        let s = g.scale;
        let cy = H / 2.0;
        g.canvas.copy_from_slice(&g.base_rgb);

        let mut texts: Vec<(String, f32, bool, Rgb)> = Vec::new(); // text, x, right-aligned, colour

        match &self.state {
            State::Hidden => return,
            State::Recording => {
                for (i, &lvl) in self.history.iter().enumerate() {
                    let h = (lvl * MAX_BAR).max(1.5);
                    let colour = if lvl < 0.02 { BAR_IDLE } else { mix(BAR_LOW, BAR_HIGH, lvl) };
                    g.bar(i, h, colour);
                }
                if self.paused {
                    g.circle(25.0 * s, cy * s, 5.0 * s, PAUSE_AMBER);
                    texts.push(("Paused".to_string(), W - 18.0, true, PAUSE_AMBER));
                } else {
                    let p = 0.5 + 0.5 * (self.frame as f32 / 6.0).sin();
                    g.circle(25.0 * s, cy * s, 5.0 * s, mix(DOT_DIM, REC_RED, p));
                    let secs = self.started.elapsed().as_secs();
                    texts.push((format!("{}:{:02}", secs / 60, secs % 60), W - 18.0, true, TEXT_DIM));
                }
            }
            State::Working => {
                let vis = BARS - g.work_start;
                let head = (self.frame as f32 * 1.1) % (vis as f32 + 10.0);
                for j in 0..vis {
                    let glow = (1.0 - (j as f32 - head).abs() / 5.0).max(0.0);
                    g.bar(g.work_start + j, 1.5 + glow * 7.0, mix(BAR_IDLE, BAR_HIGH, glow));
                }
                texts.push((WORK_LABEL.to_string(), 20.0, false, TEXT_BRIGHT));
            }
            State::Notice { text, error } => {
                let text: String = text.chars().take(46).collect();
                texts.push((text, 20.0, false, if *error { ERR_RED } else { TEXT_BRIGHT }));
            }
        }

        g.flush_canvas();
        for (text, x, right, colour) in &texts {
            g.text(text, *x, *right, *colour);
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
        let w = (W * scale).round() as i32;
        let h = (H * scale).round() as i32;
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
            let font = CreateFontW(
                -(FONT_PT * 96.0 * scale / 72.0).round() as i32,
                0,
                0,
                0,
                FW_NORMAL as _,
                0,
                0,
                0,
                DEFAULT_CHARSET as _,
                OUT_DEFAULT_PRECIS as _,
                CLIP_DEFAULT_PRECIS as _,
                CLEARTYPE_QUALITY as _,
                0,
                face.as_ptr(),
            );
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
                old_font,
                bits: bits as *mut u32,
                base_rgb: vec![[0.0; 3]; n],
                base_alpha: vec![0.0; n],
                canvas: vec![[0.0; 3]; n],
                work_start: BARS,
            };
            g.build_panel();

            let label_right = 20.0 + g.measure(WORK_LABEL).cx as f32 / scale;
            g.work_start = (0..BARS)
                .find(|&i| BARS_X0 + i as f32 * PITCH >= label_right + 14.0)
                .unwrap_or(BARS);
            g
        }
    }

    /// The empty rounded panel with a 1px edge, antialiased.
    fn build_panel(&mut self) {
        let s = self.scale;
        let (cx, cy) = (self.w as f32 / 2.0, self.h as f32 / 2.0);
        let (hw, hh) = (cx - 0.5 * s, cy - 0.5 * s);
        let r = RADIUS * s;
        let edge = s.max(1.0);
        for y in 0..self.h {
            for x in 0..self.w {
                let d = round_rect_distance(x as f32 + 0.5 - cx, y as f32 + 0.5 - cy, hw, hh, r);
                let i = (y * self.w + x) as usize;
                self.base_alpha[i] = (0.5 - d).clamp(0.0, 1.0);
                let inner = (0.5 - (d + edge)).clamp(0.0, 1.0);
                self.base_rgb[i] = mix(PANEL_EDGE, PANEL_BG, inner);
            }
        }
    }

    /// Bar `i`, `half` logical pixels above and below the centre line.
    fn bar(&mut self, i: usize, half: f32, colour: Rgb) {
        let s = self.scale;
        let x1 = (BARS_X0 + i as f32 * PITCH) * s;
        let cy = H / 2.0 * s;
        self.rect(x1, cy - half * s, x1 + BAR_W * s, cy + half * s, colour);
    }

    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, colour: Rgb) {
        let (px0, px1) = (x0.floor().max(0.0) as i32, (x1.ceil() as i32).min(self.w));
        let (py0, py1) = (y0.floor().max(0.0) as i32, (y1.ceil() as i32).min(self.h));
        for py in py0..py1 {
            let cov_y = overlap(py as f32, y0, y1);
            for px in px0..px1 {
                let a = cov_y * overlap(px as f32, x0, x1);
                self.blend(px, py, colour, a);
            }
        }
    }

    fn circle(&mut self, cx: f32, cy: f32, r: f32, colour: Rgb) {
        let (px0, px1) = ((cx - r - 1.0).max(0.0) as i32, ((cx + r + 1.0) as i32).min(self.w));
        let (py0, py1) = ((cy - r - 1.0).max(0.0) as i32, ((cy + r + 1.0) as i32).min(self.h));
        for py in py0..py1 {
            for px in px0..px1 {
                let (dx, dy) = (px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
                let a = (0.5 - ((dx * dx + dy * dy).sqrt() - r)).clamp(0.0, 1.0);
                self.blend(px, py, colour, a);
            }
        }
    }

    fn blend(&mut self, x: i32, y: i32, colour: Rgb, a: f32) {
        if a <= 0.0 {
            return;
        }
        let i = (y * self.w + x) as usize;
        self.canvas[i] = mix(self.canvas[i], colour, a);
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
    fn text(&self, text: &str, x: f32, right_aligned: bool, colour: Rgb) {
        let size = self.measure(text);
        let mut left = (x * self.scale).round() as i32;
        if right_aligned {
            left -= size.cx;
        }
        let top = (self.h - size.cy) / 2;
        let t = wide_no_nul(text);
        unsafe {
            SetTextColor(self.dc, byte(colour[2]) << 16 | byte(colour[1]) << 8 | byte(colour[0]));
            TextOutW(self.dc, left, top, t.as_ptr(), t.len() as i32);
        }
    }

    fn measure(&self, text: &str) -> SIZE {
        let t = wide_no_nul(text);
        let mut size = SIZE { cx: 0, cy: 0 };
        unsafe {
            GetTextExtentPoint32W(self.dc, t.as_ptr(), t.len() as i32, &mut size);
        }
        size
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
            DeleteObject(self.bmp);
            DeleteDC(self.dc);
        }
    }
}

fn round_rect_distance(x: f32, y: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = x.abs() - (hw - r);
    let qy = y.abs() - (hh - r);
    let (ox, oy) = (qx.max(0.0), qy.max(0.0));
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

/// How much of pixel [p, p+1] lies inside [a, b].
fn overlap(p: f32, a: f32, b: f32) -> f32 {
    ((p + 1.0).min(b) - p.max(a)).clamp(0.0, 1.0)
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn byte(v: f32) -> u32 {
    (v + 0.5).clamp(0.0, 255.0) as u32
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn wide_no_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
