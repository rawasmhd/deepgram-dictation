//! Adds the Windows version resource (product name and version) to
//! dictation.exe. Code signing needs it (#19): SignPath checks that the
//! product name and version match the project and the release.
//!
//! It needs a resource compiler. The MSVC toolchain (GitHub Actions, the
//! release build) has rc.exe; the GNU toolchain has none, so GNU builds
//! skip the resource with a warning.

use std::path::PathBuf;

const PRODUCT_NAME: &str = "Deepgram Dictation";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        println!("cargo:warning=no version resource: it needs the MSVC toolchain (the release build has it)");
        return;
    }

    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let parts: Vec<u16> = version.split(['.', '-']).filter_map(|p| p.parse().ok()).collect();
    let n = |i: usize| parts.get(i).copied().unwrap_or(0);
    let numeric = format!("{},{},{},0", n(0), n(1), n(2));

    let rc = format!(
        r#"#include <winver.h>
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
"#
    );
    let path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("version.rc");
    std::fs::write(&path, rc).unwrap();
    embed_resource::compile(&path, embed_resource::NONE).manifest_required().unwrap();
}
