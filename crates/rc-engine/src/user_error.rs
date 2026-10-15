//! Player-facing startup failures for the Windows GUI executable.
//! The launcher still receives the existing exit codes and stderr text; a direct launch also
//! gets a visible explanation and an on-disk record.

use std::io::Write;

pub fn report(message: &str) {
    eprintln!("error: {message}");
    let path = crate::render_settings::settings_path()
        .and_then(|p| p.parent().map(|dir| dir.join("startup.log")))
        .or_else(|| std::env::current_exe().ok().and_then(|p| p.parent().map(|dir| dir.join("startup.log"))));
    if let Some(path) = path {
        if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
        if let Ok(mut file) = std::fs::OpenOptions::new().append(true).create(true).open(&path) {
            let _ = writeln!(file, "Ratchet & Clank Recomp {}: {message}", env!("CARGO_PKG_VERSION"));
        }
    }
    #[cfg(windows)]
    show_dialog(message);
}

#[cfg(windows)]
fn show_dialog(message: &str) {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(hwnd: *mut std::ffi::c_void, text: *const u16, caption: *const u16, flags: u32) -> i32;
    }
    let text: Vec<u16> = std::ffi::OsStr::new(message).encode_wide().chain(Some(0)).collect();
    let title: Vec<u16> = std::ffi::OsStr::new("Ratchet & Clank — Ratchet & Clank Recomp error").encode_wide().chain(Some(0)).collect();
    // MB_OK | MB_ICONERROR | MB_TOPMOST, so an early error cannot hide behind the launcher.
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), 0x10 | 0x40000); }
}
