//! Presse-papiers (texte seulement) : pour lire à voix haute le texte sélectionné.

use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

const CF_UNICODETEXT: u32 = 13;

/// Un autre programme peut tenir le presse-papiers un instant : on réessaie.
fn open() -> bool {
    for _ in 0..10 {
        if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    false
}

pub fn sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

pub fn get_text() -> Option<String> {
    if !open() {
        return None;
    }
    let text = unsafe {
        let h = GetClipboardData(CF_UNICODETEXT);
        let p = if h.is_null() { std::ptr::null() } else { GlobalLock(h) as *const u16 };
        if p.is_null() {
            None
        } else {
            let mut n = 0;
            while *p.add(n) != 0 {
                n += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
            GlobalUnlock(h);
            Some(s)
        }
    };
    unsafe { CloseClipboard() };
    text
}

pub fn set_text(s: &str) {
    if !open() {
        return;
    }
    unsafe {
        EmptyClipboard();
        let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
        let h = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
        if !h.is_null() {
            let p = GlobalLock(h) as *mut u16;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                GlobalUnlock(h);
                SetClipboardData(CF_UNICODETEXT, h);
            }
        }
        CloseClipboard();
    }
}
