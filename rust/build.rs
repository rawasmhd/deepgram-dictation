//! Adds the Windows resources to dictation.exe:
//!
//! - The version resource (product name and version). Code signing needs
//!   it (#19): SignPath checks that the product name and version match the
//!   project and the release.
//! - The app icon, drawn by `src/art.rs` at every size Windows asks for.
//! - The manifest: modern (Common Controls 6) controls, and per-monitor
//!   DPI awareness.
//!
//! It needs a resource compiler. The MSVC toolchain (GitHub Actions, the
//! release build) has rc.exe; the GNU toolchain has none, so GNU builds
//! skip the resources with a warning.

use std::path::{Path, PathBuf};

#[path = "src/gfx.rs"]
#[allow(dead_code)]
mod gfx;
#[path = "src/art.rs"]
#[allow(dead_code)]
mod art;

const PRODUCT_NAME: &str = "Deepgram Dictation";
const ICON_SIZES: [usize; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0" xmlns:asmv3="urn:schemas-microsoft-com:asm.v3">
  <assemblyIdentity type="win32" name="DeepgramDictation" version="1.0.0.0"/>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
        processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <asmv3:application>
    <asmv3:windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </asmv3:windowsSettings>
  </asmv3:application>
</assembly>
"#;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/art.rs");
    println!("cargo:rerun-if-changed=src/gfx.rs");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let icon = out.join("app.ico");
    std::fs::write(&icon, ico(&ICON_SIZES)).unwrap();
    let manifest = out.join("app.manifest");
    std::fs::write(&manifest, MANIFEST).unwrap();

    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        println!("cargo:warning=no resources (version, icon, manifest): they need the MSVC toolchain (the release build has it)");
        return;
    }

    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let parts: Vec<u16> = version.split(['.', '-']).filter_map(|p| p.parse().ok()).collect();
    let n = |i: usize| parts.get(i).copied().unwrap_or(0);
    let numeric = format!("{},{},{},0", n(0), n(1), n(2));

    let rc = format!(
        r#"#include <winver.h>
1 ICON "{icon}"
1 24 "{manifest}"
VS_VERSION_INFO VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEOS VOS_NT_WINDOWS32
FILETYPE VFT_APP
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "ProductName", "{PRODUCT_NAME}"
      VALUE "ProductVersion", "{version}"
      VALUE "FileDescription", "{PRODUCT_NAME}"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "dictation"
      VALUE "OriginalFilename", "dictation.exe"
      VALUE "LegalCopyright", "MIT License, https://github.com/rawasmhd/deepgram-dictation"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        icon = rc_path(&icon),
        manifest = rc_path(&manifest),
    );
    let path = out.join("resources.rc");
    std::fs::write(&path, rc).unwrap();
    embed_resource::compile(&path, embed_resource::NONE).manifest_required().unwrap();
}

/// A path as an .rc string: backslashes doubled.
fn rc_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "\\\\")
}

/// An .ico file with the app icon at each size. 256 px is stored as PNG
/// (smaller); the rest as 32-bit bitmaps, which every Windows version reads.
fn ico(sizes: &[usize]) -> Vec<u8> {
    let images: Vec<(usize, Vec<u8>)> = sizes
        .iter()
        .map(|&size| {
            let c = art::app_icon(size);
            (size, if size >= 256 { png(&c) } else { bmp(&c) })
        })
        .collect();

    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]); // reserved, type 1 = icon
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len();
    for (size, data) in &images {
        let dim = if *size >= 256 { 0 } else { *size as u8 }; // 0 means 256
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += data.len();
    }
    for (_, data) in &images {
        out.extend_from_slice(data);
    }
    out
}

/// A 32-bit icon bitmap: the header, the pixels bottom-up, and an empty
/// AND mask (the alpha channel does the work).
fn bmp(c: &gfx::Canvas) -> Vec<u8> {
    let (w, h) = (c.w, c.h);
    let mask_row = w.div_ceil(32) * 4;
    let mut out = Vec::new();
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(2 * h as i32).to_le_bytes()); // colour + mask
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]); // BI_RGB, sizes and colours unused
    let px = c.bgra_straight();
    for y in (0..h).rev() {
        for &p in &px[y * w..(y + 1) * w] {
            out.extend_from_slice(&p.to_le_bytes());
        }
    }
    out.extend(std::iter::repeat(0).take(mask_row * h));
    out
}

fn png(c: &gfx::Canvas) -> Vec<u8> {
    let rgba: Vec<u8> = c
        .bgra_straight()
        .iter()
        .flat_map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8])
        .collect();
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, c.w as u32, c.h as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Best);
    encoder.write_header().unwrap().write_image_data(&rgba).unwrap();
    out
}
