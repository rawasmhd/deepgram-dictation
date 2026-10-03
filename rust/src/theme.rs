//! Windows' light and dark mode, and icons made from `art`.

use std::{mem, ptr};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::gfx::Canvas;
use crate::overlay::wide;

const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

/// Apps use dark mode (Settings > Personalization > Colors).
pub fn apps_dark() -> bool {
    read_dword("AppsUseLightTheme") == Some(0)
}

/// The taskbar is dark. This is a separate setting from the apps.
pub fn taskbar_dark() -> bool {
    read_dword("SystemUsesLightTheme") != Some(1)
}

fn read_dword(name: &str) -> Option<u32> {
    let mut value = 0u32;
    let mut size = 4u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide(PERSONALIZE).as_ptr(),
            wide(name).as_ptr(),
            RRF_RT_REG_DWORD,
            ptr::null_mut(),
            (&mut value as *mut u32).cast(),
            &mut size,
        )
    };
    (status == ERROR_SUCCESS).then_some(value)
}

/// Let menus (the tray menu) follow dark mode. Windows has no public API
/// for this; uxtheme's SetPreferredAppMode (ordinal 135) is what other
/// apps use. Without it, the menu stays light.
pub fn allow_dark_menus() {
    unsafe {
        let uxtheme = LoadLibraryW(wide("uxtheme.dll").as_ptr());
        if uxtheme.is_null() {
            return;
        }
        if let Some(f) = GetProcAddress(uxtheme, 135 as *const u8) {
            let set_preferred_app_mode: unsafe extern "system" fn(i32) -> i32 = mem::transmute(f);
            set_preferred_app_mode(1); // AllowDark: follow the system
        }
        if let Some(f) = GetProcAddress(uxtheme, 136 as *const u8) {
            let flush_menu_themes: unsafe extern "system" fn() = mem::transmute(f);
            flush_menu_themes();
        }
    }
}

/// An icon handle from a canvas. Destroy it with DestroyIcon.
pub fn icon(c: &Canvas) -> HICON {
    unsafe {
        let mut bmi: BITMAPINFO = mem::zeroed();
        bmi.bmiHeader.biSize = mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = c.w as i32;
        bmi.bmiHeader.biHeight = -(c.h as i32); // top-down rows
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB as _;
        let mut bits: *mut core::ffi::c_void = ptr::null_mut();
        let color = CreateDIBSection(ptr::null_mut(), &bmi, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
        if color.is_null() {
            return ptr::null_mut();
        }
        let px = c.bgra_straight();
        ptr::copy_nonoverlapping(px.as_ptr(), bits as *mut u32, px.len());
        let mask = CreateBitmap(c.w as i32, c.h as i32, 1, 1, ptr::null());
        let info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let icon = CreateIconIndirect(&info);
        DeleteObject(mask);
        DeleteObject(color);
        icon
    }
}
