//! The Custom words window: add and remove the terms in words.txt.
//!
//! It opens from "Custom words…" in the tray menu, or with
//! `dictation.exe --words`. It looks like the setup window and uses its
//! palette and drawing helpers. The text field is a standard EDIT control
//! and the list is an owner-drawn LISTBOX.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::{mem, ptr};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus, IsWindowEnabled, SetFocus, VK_DELETE};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::gfx::{self, hex, round_rect, Canvas, Rgb};
use crate::overlay::wide;
use crate::setup::{blit, cref, framed, measure, text, Palette, DARK, DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, LIGHT};
use crate::words::{self, Words};
use crate::{art, log, theme};

const TITLE: &str = "Custom words - Deepgram Dictation";

const ID_SAVE: usize = 1; // IDOK: Enter adds in the text field, else saves
const ID_CANCEL: usize = 2; // IDCANCEL, so Esc cancels
const ID_EDIT: usize = 100;
const ID_ADD: usize = 101;
const ID_LIST: usize = 102;
const ID_REMOVE: usize = 103;
const DC_HASDEFID: usize = 0x534B;

// Layout in logical pixels (96 dpi).
const W: f32 = 460.0;
const H: f32 = 508.0;
const PAD: f32 = 24.0;
const CARD: [f32; 4] = [24.0, 84.0, 436.0, 400.0];
const LABEL_Y: f32 = 100.0;
const INPUT: [f32; 4] = [40.0, 124.0, 332.0, 156.0];
const ADD: [f32; 4] = [340.0, 124.0, 420.0, 156.0];
const LIST: [f32; 4] = [40.0, 168.0, 420.0, 340.0];
const REMOVE: [f32; 4] = [40.0, 352.0, 136.0, 384.0];
const COUNT_Y: f32 = 359.0;
const STATUS_Y: f32 = 412.0;
const FOOTER_Y: f32 = 440.0;
const ITEM_H: f32 = 28.0;
const BUTTON_W: f32 = 96.0;
const BUTTON_H: f32 = 32.0;

/// The warning colour; the setup window has no warnings.
const WARN_LIGHT: Rgb = hex(0x9D5D00);
const WARN_DARK: Rgb = hex(0xFCE100);

/// The open window, so the tray menu can bring it to the front.
static OPEN: AtomicIsize = AtomicIsize::new(0);

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Hint,
    Warn,
    Error,
}

struct Fonts {
    title: HFONT,
    body: HFONT,
    strong: HFONT,
    small: HFONT,
}

struct Ui {
    hwnd: HWND,
    scale: f32,
    pal: &'static Palette,
    warn: Rgb,
    fonts: Fonts,
    input_brush: HBRUSH,
    edit: HWND,
    list: HWND,
    remove: HWND,
    terms: RefCell<Vec<String>>,
    /// Deepgram rejected the list that was in the file when the window opened.
    rejected: Option<Vec<String>>,
    /// A short message about the last action, until the next one.
    note: RefCell<Option<String>>,
    focused: Cell<bool>,
    /// The control with the focus when the window was deactivated.
    last_focus: Cell<HWND>,
    done: Cell<bool>,
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

/// Show the window, or bring it to the front if it is open. Returns when
/// the window closes. A saved list is used from the next dictation.
pub fn open(words: &Words) {
    let open = OPEN.load(Ordering::Relaxed) as HWND;
    if !open.is_null() {
        unsafe { SetForegroundWindow(open) };
        return;
    }
    unsafe { run(words) }
}

unsafe fn run(words: &Words) {
    let instance = GetModuleHandleW(ptr::null());
    let class = wide("DictationWordsRs");
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
    // clip the children: the window repaints its background when the list changes
    let style = WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
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
        return;
    }
    OPEN.store(hwnd as isize, Ordering::Relaxed);
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

    // the text field, inside the frame that `paint` draws
    let field = [
        px(INPUT[0] + 10.0),
        px((INPUT[1] + INPUT[3]) / 2.0 - 10.0),
        px(INPUT[2] - 8.0),
        px((INPUT[1] + INPUT[3]) / 2.0 + 10.0),
    ];
    let edit = control("EDIT", "", ES_AUTOHSCROLL as u32, field, ID_EDIT, fonts.body);
    control("BUTTON", "Add", owner_draw, ADD.map(px), ID_ADD, fonts.body);

    // the list, one pixel inside its frame
    let one = px(1.0).max(1);
    let lr = LIST.map(px);
    let list_style = (LBS_OWNERDRAWFIXED | LBS_HASSTRINGS | LBS_NOINTEGRALHEIGHT | LBS_NOTIFY | LBS_WANTKEYBOARDINPUT)
        as u32
        | WS_VSCROLL;
    let list = control("LISTBOX", "", list_style, [lr[0] + one, lr[1] + one, lr[2] - one, lr[3] - one], ID_LIST, fonts.body);
    // WM_MEASUREITEM comes before the window state exists, so set it here
    SendMessageW(list, LB_SETITEMHEIGHT, 0, px(ITEM_H) as LPARAM);
    if dark {
        // a dark scroll bar
        SetWindowTheme(list, wide("DarkMode_Explorer").as_ptr(), ptr::null());
    }
    let remove = control("BUTTON", "Remove", owner_draw, REMOVE.map(px), ID_REMOVE, fonts.body);

    let by = FOOTER_Y + (H - FOOTER_Y - BUTTON_H) / 2.0;
    let save_x = W - PAD - BUTTON_W;
    let cancel_x = save_x - 8.0 - BUTTON_W;
    let button = |x: f32| [px(x), px(by), px(x + BUTTON_W), px(by + BUTTON_H)];
    control("BUTTON", "Cancel", owner_draw, button(cancel_x), ID_CANCEL, fonts.body);
    control("BUTTON", "Save", owner_draw, button(save_x), ID_SAVE, fonts.strong);

    let terms = words::load();
    for t in &terms {
        SendMessageW(list, LB_ADDSTRING, 0, wide(t).as_ptr() as LPARAM);
    }
    let rejected = words.is_rejected(&terms).then(|| terms.clone());

    UI.with(|u| {
        *u.borrow_mut() = Some(Rc::new(Ui {
            hwnd,
            scale,
            pal,
            warn: if dark { WARN_DARK } else { WARN_LIGHT },
            fonts,
            input_brush: CreateSolidBrush(cref(pal.input)),
            edit,
            list,
            remove,
            terms: RefCell::new(terms),
            rejected,
            note: RefCell::new(None),
            focused: Cell::new(false),
            last_focus: Cell::new(edit),
            done: Cell::new(false),
        }))
    });
    if let Some(ui) = ui() {
        refresh(&ui);
    }

    ShowWindow(hwnd, SW_SHOW);
    SetForegroundWindow(hwnd);
    SetFocus(edit);

    let mut msg: MSG = mem::zeroed();
    while !ui().is_some_and(|u| u.done.get()) && GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
        // Tab, Enter and Esc, like in a dialog box
        if IsDialogMessageW(hwnd, &msg) == 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    OPEN.store(0, Ordering::Relaxed);
    if let Some(ui) = UI.with(|u| u.borrow_mut().take()) {
        let f = &ui.fonts;
        for font in [f.title, f.body, f.strong, f.small] {
            DeleteObject(font);
        }
        DeleteObject(ui.input_brush);
    }
    DestroyIcon(big);
    DestroyIcon(small);
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let Some(ui) = ui() else { return DefWindowProcW(hwnd, msg, wp, lp) };
    match msg {
        WM_COMMAND => {
            let (id, code) = (wp & 0xFFFF, ((wp >> 16) & 0xFFFF) as u32);
            match id {
                ID_SAVE if GetFocus() == ui.edit => add(&ui),
                ID_SAVE => save(&ui),
                ID_CANCEL => {
                    DestroyWindow(hwnd);
                }
                ID_ADD => add(&ui),
                ID_REMOVE => remove(&ui),
                ID_LIST if code == LBN_SELCHANGE => refresh(&ui),
                ID_EDIT if code == EN_CHANGE => {
                    if ui.note.borrow_mut().take().is_some() {
                        refresh(&ui);
                    }
                }
                ID_EDIT if code == EN_SETFOCUS || code == EN_KILLFOCUS => {
                    ui.focused.set(code == EN_SETFOCUS);
                    let r = rect_px(&ui, INPUT);
                    InvalidateRect(hwnd, &r, 0);
                }
                _ => {}
            }
            0
        }
        DM_GETDEFID => ((DC_HASDEFID << 16) | ID_SAVE) as LRESULT,
        // a plain window keeps the focus itself when it is activated; pass it
        // on to the control that had it, like a dialog box does
        WM_ACTIVATE if (wp & 0xFFFF) as u32 == WA_INACTIVE => {
            let focus = GetFocus();
            if !focus.is_null() && IsChild(hwnd, focus) != 0 {
                ui.last_focus.set(focus);
            }
            0
        }
        WM_SETFOCUS => {
            let target = ui.last_focus.get();
            let usable = IsWindowVisible(target) != 0 && IsWindowEnabled(target) != 0;
            SetFocus(if usable { target } else { ui.edit });
            0
        }
        // the Delete key in the list
        WM_VKEYTOITEM if (wp & 0xFFFF) as u16 == VK_DELETE => {
            remove(&ui);
            -2
        }
        WM_VKEYTOITEM => -1,
        WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let dc = wp as HDC;
            SetTextColor(dc, cref(ui.pal.text));
            SetBkColor(dc, cref(ui.pal.input));
            ui.input_brush as LRESULT
        }
        WM_DRAWITEM => {
            let item = &*(lp as *const DRAWITEMSTRUCT);
            if item.CtlID as usize == ID_LIST {
                draw_list_item(&ui, item);
            } else {
                draw_button(&ui, item);
            }
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

// -- editing --------------------------------------------------------------------

unsafe fn add(ui: &Ui) {
    let len = GetWindowTextLengthW(ui.edit);
    let mut buf = vec![0u16; len as usize + 1];
    GetWindowTextW(ui.edit, buf.as_mut_ptr(), buf.len() as i32);
    let term = String::from_utf16_lossy(&buf[..len as usize]).trim().to_string();
    if term.is_empty() {
        return;
    }
    if term.starts_with('#') {
        set_note(ui, "A word cannot start with #.");
        return;
    }
    let existing = ui.terms.borrow().iter().position(|t| t.eq_ignore_ascii_case(&term));
    if let Some(i) = existing {
        SendMessageW(ui.list, LB_SETCURSEL, i, 0);
        set_note(ui, &format!("\"{term}\" is already in the list."));
        return;
    }
    let i = SendMessageW(ui.list, LB_ADDSTRING, 0, wide(&term).as_ptr() as LPARAM);
    SendMessageW(ui.list, LB_SETCURSEL, i as WPARAM, 0);
    ui.terms.borrow_mut().push(term);
    SetWindowTextW(ui.edit, wide("").as_ptr());
    SetFocus(ui.edit);
    ui.note.borrow_mut().take();
    refresh(ui);
}

unsafe fn remove(ui: &Ui) {
    let i = SendMessageW(ui.list, LB_GETCURSEL, 0, 0);
    if i < 0 {
        return;
    }
    let i = i as usize;
    ui.terms.borrow_mut().remove(i);
    SendMessageW(ui.list, LB_DELETESTRING, i, 0);
    let left = ui.terms.borrow().len();
    if left > 0 {
        SendMessageW(ui.list, LB_SETCURSEL, i.min(left - 1), 0);
    } else {
        SetFocus(ui.edit);
    }
    ui.note.borrow_mut().take();
    refresh(ui);
}

unsafe fn save(ui: &Ui) {
    let terms = ui.terms.borrow().clone();
    match words::save(&terms) {
        Ok(_) => {
            DestroyWindow(ui.hwnd);
        }
        Err(e) => {
            log(&format!("custom words: could not save: {e}"));
            MessageBoxW(
                ui.hwnd,
                wide(&format!("The custom words could not be saved.\n\n{e}")).as_ptr(),
                wide(TITLE).as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}

fn set_note(ui: &Ui, text: &str) {
    *ui.note.borrow_mut() = Some(text.into());
    refresh(ui);
}

/// After a change: the Remove button, the empty list, the count and status.
fn refresh(ui: &Ui) {
    unsafe {
        let empty = ui.terms.borrow().is_empty();
        let selected = SendMessageW(ui.list, LB_GETCURSEL, 0, 0) >= 0;
        EnableWindow(ui.remove, selected.into());
        ShowWindow(ui.list, if empty { SW_HIDE } else { SW_SHOWNA });
        let r = rect_px(ui, [0.0, LIST[1], W, FOOTER_Y]);
        InvalidateRect(ui.hwnd, &r, 0);
    }
}

/// The line under the card.
fn status(ui: &Ui) -> (Kind, String) {
    if let Some(note) = ui.note.borrow().as_ref() {
        return (Kind::Error, note.clone());
    }
    let terms = ui.terms.borrow();
    if ui.rejected.as_ref() == Some(&*terms) {
        return (Kind::Error, "Deepgram rejected this list. Remove some words.".into());
    }
    match words::fill(words::estimate_tokens(&terms)) {
        words::Fill::Over => (Kind::Error, "The list is too long. Deepgram will not use it.".into()),
        words::Fill::Near => (Kind::Warn, "The list is almost full.".into()),
        words::Fill::Ok => (Kind::Hint, "Add names and words that Deepgram gets wrong. 20 to 50 work best.".into()),
    }
}

// -- painting -------------------------------------------------------------------

/// The window background: header, card, field and list frames, count,
/// status and footer.
unsafe fn paint(ui: &Ui) {
    let pal = ui.pal;
    let s = ui.scale;
    let (w, h) = (ui.px(W), ui.px(H));
    let mut c = Canvas::filled(w as usize, h as usize, pal.bg);
    let r = |v: [f32; 4]| v.map(|x| x * s);

    framed(&mut c, r(CARD), 8.0 * s, s, pal.card_edge, pal.card);

    // the text field: a frame with a line along the bottom, thicker with focus
    let f = r(INPUT);
    let rad = 4.0 * s;
    framed(&mut c, f, rad, s, pal.input_edge, pal.input);
    let (line, colour) = if ui.focused.get() { (2.0 * s, pal.accent) } else { (s, pal.input_line) };
    c.paint(f, |x, y| round_rect(x, y, f[0], f[1], f[2], f[3], rad).max(f[3] - line - y), colour, 1.0);

    framed(&mut c, r(LIST), rad, s, pal.input_edge, pal.input);

    let icon = art::app_icon(ui.px(44.0) as usize);
    c.draw(&icon, ui.px(PAD) as usize, ui.px(20.0) as usize);

    // the status icon: a circle with "!" for a warning or an error
    let (kind, message) = status(ui);
    if kind != Kind::Hint {
        let (cx, cy, rr) = ((PAD + 8.0) * s, (STATUS_Y + 9.0) * s, 6.2 * s);
        let lw = 1.4 * s;
        let colour = if kind == Kind::Warn { ui.warn } else { pal.error };
        c.paint(
            [cx - 8.0 * s, cy - 8.0 * s, cx + 8.0 * s, cy + 8.0 * s],
            |x, y| {
                gfx::outline(gfx::circle(x, y, cx, cy, rr), lw)
                    .min(gfx::line(x, y, cx, cy - 3.2 * s, cx, cy + 0.6 * s, lw))
                    .min(gfx::circle(x, y, cx, cy + 3.2 * s, 0.9 * s))
            },
            colour,
            1.0,
        );
    }

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
    text(mem_dc, fonts.title, pal.text, x_text, ui.px(20.0), "Custom words");
    text(mem_dc, fonts.small, pal.sub, x_text, ui.px(48.0), "Deepgram listens for these names and words.");
    text(mem_dc, fonts.strong, pal.text, ui.px(INPUT[0]), ui.px(LABEL_Y), "Add a word or phrase");

    let count = ui.terms.borrow().len();
    if count == 0 {
        // the list is hidden: say why the box is empty
        let msg = "No custom words yet. Add one above.";
        let tw = measure(ui.hwnd, fonts.small, msg);
        let x = ui.px((LIST[0] + LIST[2]) / 2.0) - tw / 2;
        text(mem_dc, fonts.small, pal.sub, x, ui.px((LIST[1] + LIST[3]) / 2.0 - 9.0), msg);
    }
    let count_text = if count == 1 { "1 word".to_string() } else { format!("{count} words") };
    let cw = measure(ui.hwnd, fonts.small, &count_text);
    text(mem_dc, fonts.small, pal.sub, ui.px(LIST[2]) - cw, ui.px(COUNT_Y), &count_text);

    let (colour, x) = match kind {
        Kind::Hint => (pal.sub, ui.px(PAD)),
        Kind::Warn => (ui.warn, ui.px(PAD + 24.0)),
        Kind::Error => (pal.error, ui.px(PAD + 24.0)),
    };
    text(mem_dc, fonts.small, colour, x, ui.px(STATUS_Y), &message);

    BitBlt(dc, 0, 0, w, h, mem_dc, 0, 0, SRCCOPY);
    SelectObject(mem_dc, old);
    DeleteObject(bmp);
    DeleteDC(mem_dc);
    EndPaint(ui.hwnd, &ps);
}

/// One row of the list. The selected row has an accent tint and a bar.
unsafe fn draw_list_item(ui: &Ui, item: &DRAWITEMSTRUCT) {
    let pal = ui.pal;
    let s = ui.scale;
    let rc = item.rcItem;
    let (w, h) = ((rc.right - rc.left).max(1) as f32, (rc.bottom - rc.top).max(1) as f32);
    let selected = item.itemState & ODS_SELECTED != 0 && item.itemID != u32::MAX;
    let mut c = Canvas::filled(w as usize, h as usize, pal.input);
    let all = [0.0, 0.0, w, h];
    if selected {
        let tint = gfx::mix(pal.input, pal.accent, 0.16);
        c.paint(all, |x, y| round_rect(x, y, 3.0 * s, s, w - 3.0 * s, h - s, 4.0 * s), tint, 1.0);
        c.paint(all, |x, y| round_rect(x, y, 3.0 * s, h / 2.0 - 7.0 * s, 6.0 * s, h / 2.0 + 7.0 * s, 1.5 * s), pal.accent, 1.0);
    }
    if item.itemState & ODS_FOCUS != 0 && GetFocus() == ui.list {
        c.paint(all, |x, y| gfx::outline(round_rect(x, y, 2.0 * s, s, w - 2.0 * s, h - s, 4.0 * s), s), pal.text, 0.6);
    }
    let dc = item.hDC;
    blit(dc, rc.left, rc.top, &c);
    if item.itemID == u32::MAX {
        return;
    }
    let Some(term) = ui.terms.borrow().get(item.itemID as usize).cloned() else { return };
    SetBkMode(dc, TRANSPARENT as _);
    let old = SelectObject(dc, ui.fonts.body);
    let wt: Vec<u16> = term.encode_utf16().collect();
    let mut size = SIZE { cx: 0, cy: 0 };
    GetTextExtentPoint32W(dc, wt.as_ptr(), wt.len() as i32, &mut size);
    SetTextColor(dc, cref(pal.text));
    TextOutW(dc, rc.left + ui.px(14.0), rc.top + (rc.bottom - rc.top - size.cy) / 2, wt.as_ptr(), wt.len() as i32);
    SelectObject(dc, old);
}

/// Save is the accent button; Cancel, Add and Remove are plain.
unsafe fn draw_button(ui: &Ui, item: &DRAWITEMSTRUCT) {
    let pal = ui.pal;
    let s = ui.scale;
    let rc = item.rcItem;
    let (w, h) = ((rc.right - rc.left) as f32, (rc.bottom - rc.top) as f32);
    let pressed = item.itemState & ODS_SELECTED != 0;
    let focus = item.itemState & ODS_FOCUS != 0;
    let disabled = item.itemState & ODS_DISABLED != 0;
    let id = item.CtlID as usize;
    let bg = if matches!(id, ID_SAVE | ID_CANCEL) { pal.footer } else { pal.card };
    let mut c = Canvas::filled(w as usize, h as usize, bg);
    let save = id == ID_SAVE;
    let (fill, edge) = if save { (pal.accent, pal.accent_edge) } else { (pal.button, pal.button_edge) };
    let fill = if pressed { gfx::mix(fill, edge, 0.6) } else { fill };
    framed(&mut c, [s, s, w - s, h - s], 4.0 * s, s, edge, fill);
    if focus {
        c.paint([0.0, 0.0, w, h], |x, y| gfx::outline(round_rect(x, y, s, s, w - s, h - s, 4.0 * s), 2.0 * s), pal.text, 1.0);
    }
    let dc = item.hDC;
    blit(dc, rc.left, rc.top, &c);

    let mut buf = [0u16; 32];
    let n = GetWindowTextW(item.hwndItem, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
    let font = if save { ui.fonts.strong } else { ui.fonts.body };
    let colour = match () {
        _ if save => pal.on_accent,
        _ if disabled => gfx::mix(pal.sub, pal.button, 0.45),
        _ => pal.text,
    };
    SetBkMode(dc, TRANSPARENT as _);
    let old = SelectObject(dc, font);
    let mut size = SIZE { cx: 0, cy: 0 };
    GetTextExtentPoint32W(dc, buf.as_ptr(), n as i32, &mut size);
    SetTextColor(dc, cref(colour));
    let left = rc.left + (rc.right - rc.left - size.cx) / 2;
    let top = rc.top + (rc.bottom - rc.top - size.cy) / 2;
    TextOutW(dc, left, top, buf.as_ptr(), n as i32);
    SelectObject(dc, old);
}

fn rect_px(ui: &Ui, r: [f32; 4]) -> RECT {
    RECT { left: ui.px(r[0]), top: ui.px(r[1]), right: ui.px(r[2]), bottom: ui.px(r[3]) }
}
