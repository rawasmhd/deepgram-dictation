//! The setup window: the API key and autostart.
//!
//! It opens when no key is found, with `dictation.exe --setup`, or from
//! Settings in the tray menu. It checks the key with Deepgram before the
//! key is saved.
//!
//! The window paints itself (the header, the card, the status line and the
//! footer) and draws its buttons, so it looks the same on Windows 10 and 11,
//! in light and dark mode. Only the key field is a standard EDIT control.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::{mem, ptr};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::art;
use crate::deepgram::{self, Error};
use crate::gfx::{self, hex, round_rect, Canvas, Rgb};
use crate::overlay::wide;
use crate::theme;

const TITLE: &str = "Deepgram Dictation";
const KEYS_URL: &str = "https://console.deepgram.com";

const ID_SAVE: usize = 1; // IDOK, so Enter saves
const ID_CANCEL: usize = 2; // IDCANCEL, so Esc cancels
const ID_EDIT: usize = 100;
const ID_EYE: usize = 101;
const ID_LINK: usize = 102;
const ID_TOGGLE: usize = 103;
const DC_HASDEFID: usize = 0x534B;
const BULLET: usize = 0x25CF;

// DWM window attributes (Windows 10 2004 and later; older ones ignore them)
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
const DWMWA_CAPTION_COLOR: u32 = 35;

// Layout in logical pixels (96 dpi).
const W: f32 = 460.0;
const H: f32 = 372.0;
const PAD: f32 = 24.0;
const CARD: [f32; 4] = [24.0, 84.0, 436.0, 252.0];
const INPUT: [f32; 4] = [40.0, 126.0, 420.0, 158.0];
const HELP_Y: f32 = 166.0;
const SEPARATOR_Y: f32 = 196.0;
const TOGGLE: [f32; 4] = [36.0, 206.0, 424.0, 240.0];
const STATUS_Y: f32 = 268.0;
const FOOTER_Y: f32 = 304.0;
const BUTTON_W: f32 = 96.0;
const BUTTON_H: f32 = 32.0;

pub struct Choice {
    pub key: String,
    pub autostart: bool,
}

struct Palette {
    bg: Rgb,
    card: Rgb,
    card_edge: Rgb,
    text: Rgb,
    sub: Rgb,
    input: Rgb,
    input_edge: Rgb,
    input_line: Rgb,
    footer: Rgb,
    footer_edge: Rgb,
    accent: Rgb,
    accent_edge: Rgb,
    on_accent: Rgb,
    button: Rgb,
    button_edge: Rgb,
    link: Rgb,
    error: Rgb,
}

const LIGHT: Palette = Palette {
    bg: hex(0xF3F3F3),
    card: hex(0xFBFBFB),
    card_edge: hex(0xE5E5E5),
    text: hex(0x1A1A1A),
    sub: hex(0x5C5C5C),
    input: hex(0xFFFFFF),
    input_edge: hex(0xE0E0E0),
    input_line: hex(0x868686),
    footer: hex(0xEBEBEB),
    footer_edge: hex(0xE0E0E0),
    accent: hex(0x0B7A68),
    accent_edge: hex(0x0A6A5A),
    on_accent: hex(0xFFFFFF),
    button: hex(0xFBFBFB),
    button_edge: hex(0xD1D1D1),
    link: hex(0x0B7A68),
    error: hex(0xC42B1C),
};

const DARK: Palette = Palette {
    bg: hex(0x202020),
    card: hex(0x2B2B2B),
    card_edge: hex(0x383838),
    text: hex(0xFFFFFF),
    sub: hex(0xC5C5C5),
    input: hex(0x1F1F1F),
    input_edge: hex(0x3A3A3A),
    input_line: hex(0x9A9A9A),
    footer: hex(0x1C1C1C),
    footer_edge: hex(0x2E2E2E),
    accent: hex(0x4CC2A8),
    accent_edge: hex(0x6FDCC4),
    on_accent: hex(0x08201B),
    button: hex(0x2D2D2D),
    button_edge: hex(0x3F3F3F),
    link: hex(0x5FD4BA),
    error: hex(0xFF99A4),
};

#[derive(Clone, Copy, PartialEq)]
enum Status {
    None,
    Busy,
    Error,
}

struct Fonts {
    title: HFONT,
    body: HFONT,
    strong: HFONT,
    small: HFONT,
    help: HFONT,
}

/// Everything the window procedure needs. Shared through an Rc, so a
/// nested message (a message box, an EDIT notification) can read it too.
struct Ui {
    hwnd: HWND,
    scale: f32,
    pal: &'static Palette,
    fonts: Fonts,
    input_brush: HBRUSH,
    edit: HWND,
    eye: HWND,
    toggle: HWND,
    help_split: (i32, i32), // the link's left and right edge, in pixels
    header: String,
    autostart: Cell<bool>,
    shown: Cell<bool>,
    focused: Cell<bool>,
    status: RefCell<(Status, String)>,
    done: Cell<bool>,
    result: RefCell<Option<Choice>>,
}

impl Ui {
    fn px(&self, v: f32) -> i32 {
        (v * self.scale).round() as i32
    }
}

thread_local! {
    static UI: RefCell<Option<Rc<Ui>>> = const { RefCell::new(None) };
}

fn ui() -> Option<Rc<Ui>> {
    UI.with(|u| u.borrow().clone())
}

/// Show the window and wait. None if the user cancelled. `key` fills in
/// the current key, when there is one.
pub fn run(key: Option<&str>, autostart: bool, first_run: bool) -> Option<Choice> {
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let class = wide("DictationSetupRs");
        let big = theme::icon(&art::app_icon(GetSystemMetrics(SM_CXICON) as usize));
        let small = theme::icon(&art::app_icon(GetSystemMetrics(SM_CXSMICON) as usize));
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: big,
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);

        let scale = GetDpiForSystem() as f32 / 96.0;
        let px = |v: f32| (v * scale).round() as i32;
        let style = WS_CAPTION | WS_SYSMENU;
        let mut rect = RECT { left: 0, top: 0, right: px(W), bottom: px(H) };
        AdjustWindowRectEx(&mut rect, style, 0, 0);
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide(TITLE).as_ptr(),
            style,
            (GetSystemMetrics(SM_CXSCREEN) - w) / 2,
            (GetSystemMetrics(SM_CYSCREEN) - h) / 2,
            w,
            h,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        );
        if hwnd.is_null() {
            DestroyIcon(big);
            DestroyIcon(small);
            return None;
        }
        SendMessageW(hwnd, WM_SETICON, ICON_BIG as WPARAM, big as LPARAM);
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as WPARAM, small as LPARAM);

        let dark = theme::apps_dark();
        let pal: &'static Palette = if dark { &DARK } else { &LIGHT };
        let on: BOOL = dark.into();
        DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, (&on as *const BOOL).cast(), 4);
        let caption = cref(pal.bg);
        DwmSetWindowAttribute(hwnd, DWMWA_CAPTION_COLOR, (&caption as *const u32).cast(), 4);

        let font = |size: f32, weight: u32| {
            CreateFontW(
                -px(size), 0, 0, 0, weight as _, 0, 0, 0, DEFAULT_CHARSET as _,
                OUT_DEFAULT_PRECIS as _, CLIP_DEFAULT_PRECIS as _, CLEARTYPE_QUALITY as _, 0,
                wide("Segoe UI").as_ptr(),
            )
        };
        let fonts = Fonts {
            title: font(20.0, FW_SEMIBOLD),
            body: font(14.0, FW_NORMAL),
            strong: font(14.0, FW_SEMIBOLD),
            small: font(13.0, FW_NORMAL),
            help: font(12.0, FW_NORMAL),
        };

        let control = |class: &str, text: &str, style: u32, r: [i32; 4], id: usize, f: HFONT| {
            let c = CreateWindowExW(
                0,
                wide(class).as_ptr(),
                wide(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | style,
                r[0],
                r[1],
                r[2] - r[0],
                r[3] - r[1],
                hwnd,
                id as HMENU,
                instance,
                ptr::null(),
            );
            SendMessageW(c, WM_SETFONT, f as WPARAM, 1);
            c
        };
        let owner_draw = BS_OWNERDRAW as u32;

        // the key field, inside the frame that `paint` draws
        let eye_w = px(34.0);
        let field = [
            px(INPUT[0] + 10.0),
            px((INPUT[1] + INPUT[3]) / 2.0 - 10.0),
            px(INPUT[2]) - eye_w - px(2.0),
            px((INPUT[1] + INPUT[3]) / 2.0 + 10.0),
        ];
        let edit = control("EDIT", key.unwrap_or(""), ES_AUTOHSCROLL as u32 | ES_PASSWORD as u32, field, ID_EDIT, fonts.body);
        SendMessageW(edit, EM_SETPASSWORDCHAR, BULLET, 0);
        let eye_r = [px(INPUT[2]) - eye_w, px(INPUT[1]) + px(2.0), px(INPUT[2]) - px(2.0), px(INPUT[3]) - px(2.0)];
        let eye = control("BUTTON", "Show key", owner_draw, eye_r, ID_EYE, fonts.body);

        // "Get a key at console.deepgram.com, under API Keys." with a link
        let lead = measure(hwnd, fonts.help, "Get a key at ");
        let link_w = measure(hwnd, fonts.help, "console.deepgram.com");
        let link_x = px(INPUT[0]) + lead;
        let link_r = [link_x, px(HELP_Y) - px(1.0), link_x + link_w, px(HELP_Y + 18.0)];
        control("BUTTON", "console.deepgram.com", owner_draw, link_r, ID_LINK, fonts.help);

        let toggle_r = TOGGLE.map(px);
        let toggle = control("BUTTON", "Start when I sign in to Windows", owner_draw, toggle_r, ID_TOGGLE, fonts.body);

        let by = FOOTER_Y + (H - FOOTER_Y - BUTTON_H) / 2.0;
        let save_x = W - PAD - BUTTON_W;
        let cancel_x = save_x - 8.0 - BUTTON_W;
        let button = |x: f32| [px(x), px(by), px(x + BUTTON_W), px(by + BUTTON_H)];
        control("BUTTON", "Cancel", owner_draw, button(cancel_x), ID_CANCEL, fonts.body);
        control("BUTTON", "Save", owner_draw, button(save_x), ID_SAVE, fonts.strong);

        let input_brush = CreateSolidBrush(cref(pal.input));
        UI.with(|u| {
            *u.borrow_mut() = Some(Rc::new(Ui {
                hwnd,
                scale,
                pal,
                fonts,
                input_brush,
                edit,
                eye,
                toggle,
                help_split: (link_x, link_x + link_w),
                header: if first_run { "Set up dictation" } else { "Settings" }.into(),
                autostart: Cell::new(autostart),
                shown: Cell::new(false),
                focused: Cell::new(false),
                status: RefCell::new((Status::None, String::new())),
                done: Cell::new(false),
                result: RefCell::new(None),
            }))
        });

        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetFocus(edit);
        SendMessageW(edit, EM_SETSEL, 0, -1);

        let mut msg: MSG = mem::zeroed();
        while !ui().is_some_and(|u| u.done.get()) && GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            // Tab, Enter and Esc, like in a dialog box
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        let ui = UI.with(|u| u.borrow_mut().take());
        let result = ui.as_ref().and_then(|u| u.result.borrow_mut().take());
        if let Some(ui) = ui {
            let f = &ui.fonts;
            for font in [f.title, f.body, f.strong, f.small, f.help] {
                DeleteObject(font);
            }
            DeleteObject(ui.input_brush);
        }
        DestroyIcon(big);
        DestroyIcon(small);
        result
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let Some(ui) = ui() else { return DefWindowProcW(hwnd, msg, wp, lp) };
    match msg {
        WM_COMMAND => {
            let (id, code) = (wp & 0xFFFF, (wp >> 16) & 0xFFFF);
            match id {
                ID_SAVE => save(&ui),
                ID_CANCEL => {
                    DestroyWindow(hwnd);
                }
                ID_EYE => {
                    let shown = !ui.shown.get();
                    ui.shown.set(shown);
                    SendMessageW(ui.edit, EM_SETPASSWORDCHAR, if shown { 0 } else { BULLET }, 0);
                    SetWindowTextW(ui.eye, wide(if shown { "Hide key" } else { "Show key" }).as_ptr());
                    InvalidateRect(ui.edit, ptr::null(), 1);
                    InvalidateRect(ui.eye, ptr::null(), 0);
                }
                ID_LINK => {
                    ShellExecuteW(hwnd, wide("open").as_ptr(), wide(KEYS_URL).as_ptr(), ptr::null(), ptr::null(), SW_SHOWNORMAL);
                }
                ID_TOGGLE => {
                    ui.autostart.set(!ui.autostart.get());
                    InvalidateRect(ui.toggle, ptr::null(), 0);
                }
                ID_EDIT if code == EN_SETFOCUS as usize || code == EN_KILLFOCUS as usize => {
                    ui.focused.set(code == EN_SETFOCUS as usize);
                    let r = rect_px(&ui, INPUT);
                    InvalidateRect(hwnd, &r, 0);
                }
                _ => {}
            }
            0
        }
        DM_GETDEFID => ((DC_HASDEFID << 16) | ID_SAVE) as LRESULT,
        WM_CTLCOLOREDIT => {
            let dc = wp as HDC;
            SetTextColor(dc, cref(ui.pal.text));
            SetBkColor(dc, cref(ui.pal.input));
            ui.input_brush as LRESULT
        }
        WM_SETCURSOR if GetDlgCtrlID(wp as HWND) == ID_LINK as i32 => {
            SetCursor(LoadCursorW(ptr::null_mut(), IDC_HAND));
            1
        }
        WM_DRAWITEM => {
            draw_item(&ui, &*(lp as *const DRAWITEMSTRUCT));
            1
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint(&ui);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            ui.done.set(true);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

// -- saving -----------------------------------------------------------------------

unsafe fn save(ui: &Ui) {
    let len = GetWindowTextLengthW(ui.edit);
    let mut buf = vec![0u16; len as usize + 1];
    GetWindowTextW(ui.edit, buf.as_mut_ptr(), buf.len() as i32);
    let key = String::from_utf16_lossy(&buf[..len as usize]).trim().to_string();
    if key.is_empty() {
        set_status(ui, Status::Error, "Enter a key first.");
        return;
    }

    set_status(ui, Status::Busy, "Checking the key with Deepgram…");
    match deepgram::check_key(&key) {
        Ok(()) => {}
        Err(Error::KeyRejected) => {
            set_status(ui, Status::Error, "Deepgram rejected this key. Check it and try again.");
            return;
        }
        Err(e) => {
            set_status(ui, Status::None, "");
            let answer = MessageBoxW(
                ui.hwnd,
                wide(&format!("Could not check the key ({e}).\n\nSave it anyway?")).as_ptr(),
                wide(TITLE).as_ptr(),
                MB_YESNO | MB_ICONWARNING,
            );
            if answer != IDYES {
                return;
            }
        }
    }
    *ui.result.borrow_mut() = Some(Choice { key, autostart: ui.autostart.get() });
    DestroyWindow(ui.hwnd);
}

fn set_status(ui: &Ui, kind: Status, text: &str) {
    *ui.status.borrow_mut() = (kind, text.into());
    unsafe {
        let r = RECT { left: 0, top: ui.px(STATUS_Y - 4.0), right: ui.px(W), bottom: ui.px(FOOTER_Y) };
        InvalidateRect(ui.hwnd, &r, 0);
        UpdateWindow(ui.hwnd);
    }
}

// -- painting ---------------------------------------------------------------------

/// The window background: header, card, key frame, status and footer.
unsafe fn paint(ui: &Ui) {
    let pal = ui.pal;
    let s = ui.scale;
    let (w, h) = (ui.px(W), ui.px(H));
    let mut c = Canvas::filled(w as usize, h as usize, pal.bg);
    let r = |v: [f32; 4]| v.map(|x| x * s);

    // the card
    let card = r(CARD);
    framed(&mut c, card, 8.0 * s, s, pal.card_edge, pal.card);
    let sep = SEPARATOR_Y * s;
    c.paint([card[0] + 16.0 * s, sep, card[2] - 16.0 * s, sep + s], |x, y| {
        gfx::round_rect(x, y, card[0] + 16.0 * s, sep, card[2] - 16.0 * s, sep + s.max(1.0), 0.0)
    }, pal.card_edge, 1.0);

    // the key field: a frame with a line along the bottom, thicker with focus
    let f = r(INPUT);
    let rad = 4.0 * s;
    framed(&mut c, f, rad, s, pal.input_edge, pal.input);
    let (line, colour) = if ui.focused.get() { (2.0 * s, pal.accent) } else { (s, pal.input_line) };
    c.paint(f, |x, y| round_rect(x, y, f[0], f[1], f[2], f[3], rad).max(f[3] - line - y), colour, 1.0);

    // the header icon
    let icon = art::app_icon(ui.px(44.0) as usize);
    c.draw(&icon, ui.px(PAD) as usize, ui.px(20.0) as usize);

    // the status icon
    let (kind, message) = ui.status.borrow().clone();
    let (cx, cy, rr) = ((PAD + 8.0) * s, (STATUS_Y + 9.0) * s, 6.2 * s);
    let lw = 1.4 * s;
    match kind {
        Status::None => {}
        Status::Busy => c.paint(
            [cx - 8.0 * s, cy - 8.0 * s, cx + 8.0 * s, cy + 8.0 * s],
            // a ring with a gap at the top right
            |x, y| {
                let d = gfx::outline(gfx::circle(x, y, cx, cy, rr), lw);
                if x > cx && y < cy { d.max(0.6) } else { d }
            },
            pal.accent,
            1.0,
        ),
        Status::Error => c.paint(
            [cx - 8.0 * s, cy - 8.0 * s, cx + 8.0 * s, cy + 8.0 * s],
            |x, y| {
                gfx::outline(gfx::circle(x, y, cx, cy, rr), lw)
                    .min(gfx::line(x, y, cx, cy - 3.2 * s, cx, cy + 0.6 * s, lw))
                    .min(gfx::circle(x, y, cx, cy + 3.2 * s, 0.9 * s))
            },
            pal.error,
            1.0,
        ),
    }

    // the footer
    let fy = FOOTER_Y * s;
    c.paint([0.0, fy, w as f32, h as f32], |_, y| fy - y, pal.footer, 1.0);
    c.paint([0.0, fy, w as f32, fy + s], |_, y| (fy - y).max(y - fy - s.max(1.0)), pal.footer_edge, 1.0);

    // to the screen, through a memory DC, then the text on top
    let mut ps: PAINTSTRUCT = mem::zeroed();
    let dc = BeginPaint(ui.hwnd, &mut ps);
    let mem_dc = CreateCompatibleDC(dc);
    let bmp = CreateCompatibleBitmap(dc, w, h);
    let old = SelectObject(mem_dc, bmp);
    blit(mem_dc, 0, 0, &c);
    SetBkMode(mem_dc, TRANSPARENT as _);

    let fonts = &ui.fonts;
    let x_text = ui.px(PAD + 44.0 + 14.0);
    text(mem_dc, fonts.title, pal.text, x_text, ui.px(20.0), &ui.header);
    text(mem_dc, fonts.small, pal.sub, x_text, ui.px(48.0), "Press Alt+M in any app, speak, then press Alt+M again.");
    text(mem_dc, fonts.strong, pal.text, ui.px(INPUT[0]), ui.px(100.0), "Deepgram API key");
    text(mem_dc, fonts.help, pal.sub, ui.px(INPUT[0]), ui.px(HELP_Y), "Get a key at ");
    text(mem_dc, fonts.help, pal.sub, ui.help_split.1, ui.px(HELP_Y), ", under API Keys.");
    if kind != Status::None {
        let colour = if kind == Status::Error { pal.error } else { pal.sub };
        text(mem_dc, fonts.small, colour, ui.px(PAD + 24.0), ui.px(STATUS_Y), &message);
    }

    BitBlt(dc, 0, 0, w, h, mem_dc, 0, 0, SRCCOPY);
    SelectObject(mem_dc, old);
    DeleteObject(bmp);
    DeleteDC(mem_dc);
    EndPaint(ui.hwnd, &ps);
}

/// The buttons, the eye, the link and the toggle.
unsafe fn draw_item(ui: &Ui, item: &DRAWITEMSTRUCT) {
    let pal = ui.pal;
    let s = ui.scale;
    let rc = item.rcItem;
    let (w, h) = ((rc.right - rc.left) as f32, (rc.bottom - rc.top) as f32);
    let pressed = item.itemState & ODS_SELECTED != 0;
    let focus = item.itemState & ODS_FOCUS != 0;
    let id = item.CtlID as usize;
    let bg = match id {
        ID_SAVE | ID_CANCEL => pal.footer,
        ID_EYE => pal.input,
        _ => pal.card,
    };
    let mut c = Canvas::filled(w as usize, h as usize, bg);
    let all = [0.0, 0.0, w, h];
    let ring = |c: &mut Canvas, x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
        c.paint(all, |x, y| gfx::outline(round_rect(x, y, x0, y0, x1, y1, r), 2.0 * s), pal.text, 1.0);
    };
    let lw = 1.3 * s;
    let mut label: Option<(HFONT, Rgb, i32, bool)> = None; // font, colour, x, centred

    match id {
        ID_SAVE | ID_CANCEL => {
            let save = id == ID_SAVE;
            let (fill, edge) = if save { (pal.accent, pal.accent_edge) } else { (pal.button, pal.button_edge) };
            let fill = if pressed { gfx::mix(fill, edge, 0.6) } else { fill };
            framed(&mut c, [s, s, w - s, h - s], 4.0 * s, s, edge, fill);
            if focus {
                ring(&mut c, s, s, w - s, h - s, 4.0 * s);
            }
            let font = if save { ui.fonts.strong } else { ui.fonts.body };
            label = Some((font, if save { pal.on_accent } else { pal.text }, 0, true));
        }
        ID_EYE => {
            let (cx, cy) = (w / 2.0, h / 2.0);
            let k = 6.5 * s; // half the eye's width
            c.paint(all, |x, y| {
                // two circle arcs make the eye's outline
                let lens = gfx::circle(x, y, cx, cy + 6.0 * s, 9.0 * s).max(gfx::circle(x, y, cx, cy - 6.0 * s, 9.0 * s));
                let mut d = gfx::outline(lens, lw).min(gfx::outline(gfx::circle(x, y, cx, cy, 2.0 * s), lw));
                if ui.shown.get() {
                    d = d.min(gfx::line(x, y, cx - k, cy + k, cx + k, cy - k, lw));
                }
                d
            }, if pressed { pal.text } else { pal.sub }, 1.0);
            if focus {
                ring(&mut c, s, s, w - s, h - s, 4.0 * s);
            }
        }
        ID_LINK => {
            if focus {
                // an underline: the ring would cover the text
                c.paint(all, |x, y| round_rect(x, y, 0.0, h - 2.0 * s, w, h - 0.5 * s, 0.0), pal.link, 1.0);
            }
            label = Some((ui.fonts.help, pal.link, 0, false));
        }
        ID_TOGGLE => {
            let on = ui.autostart.get();
            let (x1, cy) = (w - 4.0 * s, h / 2.0);
            let (x0, y0, y1) = (x1 - 40.0 * s, cy - 10.0 * s, cy + 10.0 * s);
            let pill = |x: f32, y: f32| round_rect(x, y, x0, y0, x1, y1, 10.0 * s);
            if on {
                c.paint(all, pill, pal.accent, 1.0);
                c.paint(all, |x, y| gfx::circle(x, y, x1 - 10.0 * s, cy, 6.0 * s), pal.on_accent, 1.0);
            } else {
                c.paint(all, |x, y| gfx::outline(pill(x, y) + 0.5 * s, s), pal.sub, 1.0);
                c.paint(all, |x, y| gfx::circle(x, y, x0 + 10.0 * s, cy, 5.0 * s), pal.sub, 1.0);
            }
            if focus {
                ring(&mut c, x0 - 3.0 * s, y0 - 3.0 * s, x1 + 3.0 * s, y1 + 3.0 * s, 13.0 * s);
            }
            label = Some((ui.fonts.body, pal.text, ui.px(4.0), false));
        }
        _ => {}
    }

    let dc = item.hDC;
    blit(dc, rc.left, rc.top, &c);
    if let Some((font, colour, x, centred)) = label {
        let mut buf = [0u16; 64];
        let n = GetWindowTextW(item.hwndItem, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        let t = String::from_utf16_lossy(&buf[..n]);
        SetBkMode(dc, TRANSPARENT as _);
        let old = SelectObject(dc, font);
        let mut size = SIZE { cx: 0, cy: 0 };
        let wt: Vec<u16> = t.encode_utf16().collect();
        GetTextExtentPoint32W(dc, wt.as_ptr(), wt.len() as i32, &mut size);
        let left = if centred { rc.left + (rc.right - rc.left - size.cx) / 2 } else { rc.left + x };
        let top = rc.top + (rc.bottom - rc.top - size.cy) / 2;
        SetTextColor(dc, cref(colour));
        TextOutW(dc, left, top, wt.as_ptr(), wt.len() as i32);
        SelectObject(dc, old);
    }
}

/// A rounded rectangle with a `edge`-wide border.
fn framed(c: &mut Canvas, r: [f32; 4], radius: f32, edge: f32, edge_colour: Rgb, fill: Rgb) {
    let edge = edge.max(1.0);
    c.paint(r, |x, y| round_rect(x, y, r[0], r[1], r[2], r[3], radius), edge_colour, 1.0);
    c.paint(r, |x, y| round_rect(x, y, r[0] + edge, r[1] + edge, r[2] - edge, r[3] - edge, radius - edge), fill, 1.0);
}

unsafe fn blit(dc: HDC, x: i32, y: i32, c: &Canvas) {
    let mut bmi: BITMAPINFO = mem::zeroed();
    bmi.bmiHeader.biSize = mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = c.w as i32;
    bmi.bmiHeader.biHeight = -(c.h as i32);
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB as _;
    let px = c.bgra_premultiplied();
    SetDIBitsToDevice(dc, x, y, c.w as u32, c.h as u32, 0, 0, 0, c.h as u32, px.as_ptr().cast(), &bmi, DIB_RGB_COLORS);
}

unsafe fn text(dc: HDC, font: HFONT, colour: Rgb, x: i32, y: i32, s: &str) {
    let old = SelectObject(dc, font);
    SetTextColor(dc, cref(colour));
    let w: Vec<u16> = s.encode_utf16().collect();
    TextOutW(dc, x, y, w.as_ptr(), w.len() as i32);
    SelectObject(dc, old);
}

unsafe fn measure(hwnd: HWND, font: HFONT, s: &str) -> i32 {
    let dc = GetDC(hwnd);
    let old = SelectObject(dc, font);
    let w: Vec<u16> = s.encode_utf16().collect();
    let mut size = SIZE { cx: 0, cy: 0 };
    GetTextExtentPoint32W(dc, w.as_ptr(), w.len() as i32, &mut size);
    SelectObject(dc, old);
    ReleaseDC(hwnd, dc);
    size.cx
}

fn rect_px(ui: &Ui, r: [f32; 4]) -> RECT {
    RECT { left: ui.px(r[0]), top: ui.px(r[1]), right: ui.px(r[2]), bottom: ui.px(r[3]) }
}

/// A colour as a GDI COLORREF (0x00BBGGRR).
fn cref(c: Rgb) -> u32 {
    gfx::byte(c[2]) << 16 | gfx::byte(c[1]) << 8 | gfx::byte(c[0])
}
