//! Appels Windows : lancement des jeux, icônes des .exe, fenêtre.

use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::ptr::{null, null_mut};

use eframe::egui::ColorImage;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS,
};
use windows_sys::Win32::System::Threading::{GetCurrentThreadId, OpenProcess, WaitForSingleObject, INFINITE};
use windows_sys::Win32::UI::Shell::{
    SHDefExtractIconW, SHGetFileInfoW, ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOCLOSEPROCESS,
    SHELLEXECUTEINFOW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, EnumThreadWindows, GetIconInfo, IsWindowVisible, SetForegroundWindow, ShowWindow, HICON, ICONINFO,
    SW_RESTORE, SW_SHOWNORMAL,
};

fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// Lance un jeu comme un double-clic dans l'explorateur (.exe, .bat, .lnk, steam://…).
/// Renvoie le handle du processus quand Windows en fournit un (pour mesurer le temps de jeu).
pub fn shell_launch(file: &str, args: &str, dir: Option<&str>) -> Result<Option<isize>, String> {
    let file_w = wide(file);
    let args_w = wide(args);
    let dir_w = dir.map(wide);
    unsafe {
        let mut info: SHELLEXECUTEINFOW = zeroed();
        info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI;
        info.lpFile = file_w.as_ptr();
        info.lpParameters = if args.trim().is_empty() { null() } else { args_w.as_ptr() };
        info.lpDirectory = dir_w.as_ref().map_or(null(), |d| d.as_ptr());
        info.nShow = SW_SHOWNORMAL as _;
        if ShellExecuteExW(&mut info) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(if info.hProcess.is_null() { None } else { Some(info.hProcess as isize) })
    }
}

/// Attend la fin du processus puis libère le handle (à appeler depuis un thread dédié).
pub fn wait_and_close(process: isize) {
    unsafe {
        WaitForSingleObject(process as HANDLE, INFINITE);
        CloseHandle(process as HANDLE);
    }
}

/// Attend (au plus `timeout_ms`) la fin d'un processus.
pub fn wait_for_process(pid: u32, timeout_ms: u32) {
    const SYNCHRONIZE: u32 = 0x0010_0000;
    unsafe {
        let h = OpenProcess(SYNCHRONIZE, 0, pid);
        if !h.is_null() {
            WaitForSingleObject(h, timeout_ms);
            CloseHandle(h);
        }
    }
}

pub fn open_in_explorer(path: &str) {
    let p = Path::new(path);
    let arg = if p.is_file() { format!("/select,\"{path}\"") } else { format!("\"{path}\"") };
    let _ = std::process::Command::new("explorer.exe").raw_arg(arg).spawn();
}

// ------------------------------------------------------------------ fenêtre

unsafe extern "system" fn find_visible(hwnd: HWND, lparam: LPARAM) -> i32 {
    if IsWindowVisible(hwnd) != 0 {
        *(lparam as *mut isize) = hwnd as isize;
        return 0;
    }
    1
}

/// Fenêtre principale du launcher (créée par ce thread). 0 si pas encore visible.
pub fn own_window() -> isize {
    let mut found: isize = 0;
    unsafe {
        EnumThreadWindows(GetCurrentThreadId(), Some(find_visible), &mut found as *mut isize as LPARAM);
    }
    found
}

pub fn restore_window(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        ShowWindow(hwnd as HWND, SW_RESTORE);
        SetForegroundWindow(hwnd as HWND);
    }
}

// ------------------------------------------------------------------ icônes

/// Extrait l'icône d'un fichier (256 px si l'exe en contient une).
pub fn extract_icon(path: &str) -> Option<ColorImage> {
    if path.is_empty() || !Path::new(path).is_file() {
        return None;
    }
    let w = wide(path);
    let ext = Path::new(path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    unsafe {
        let mut icon: HICON = null_mut();
        if matches!(ext.as_str(), "exe" | "dll" | "ico") {
            let mut large: HICON = null_mut();
            let mut small: HICON = null_mut();
            if SHDefExtractIconW(w.as_ptr(), 0, 0, &mut large, &mut small, (16 << 16) | 256) == 0 {
                icon = large;
                if !small.is_null() {
                    DestroyIcon(small);
                }
            }
        }
        if icon.is_null() {
            let mut info: SHFILEINFOW = zeroed();
            SHGetFileInfoW(w.as_ptr(), 0, &mut info, size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_LARGEICON);
            icon = info.hIcon;
        }
        if icon.is_null() {
            return None;
        }
        let img = icon_to_image(icon);
        DestroyIcon(icon);
        img
    }
}

unsafe fn icon_to_image(icon: HICON) -> Option<ColorImage> {
    let mut ii: ICONINFO = zeroed();
    if GetIconInfo(icon, &mut ii) == 0 {
        return None;
    }
    let result = (|| {
        if ii.hbmColor.is_null() {
            return None;
        }
        let mut bm: BITMAP = zeroed();
        if GetObjectW(ii.hbmColor, size_of::<BITMAP>() as i32, &mut bm as *mut BITMAP as *mut c_void) == 0 {
            return None;
        }
        let (w, h) = (bm.bmWidth, bm.bmHeight);
        if w <= 0 || h <= 0 {
            return None;
        }
        let mut bi: BITMAPINFO = zeroed();
        bi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = -h; // de haut en bas
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB as _;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let dc = CreateCompatibleDC(null_mut());
        let lines = GetDIBits(dc, ii.hbmColor, 0, h as u32, buf.as_mut_ptr() as *mut c_void, &mut bi, DIB_RGB_COLORS);
        DeleteDC(dc);
        if lines == 0 {
            return None;
        }
        let has_alpha = buf.chunks_exact(4).any(|p| p[3] != 0);
        for p in buf.chunks_exact_mut(4) {
            p.swap(0, 2); // BGRA -> RGBA
            if !has_alpha {
                p[3] = 255;
            }
        }
        Some(ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &buf))
    })();
    if !ii.hbmColor.is_null() {
        DeleteObject(ii.hbmColor);
    }
    if !ii.hbmMask.is_null() {
        DeleteObject(ii.hbmMask);
    }
    result
}
