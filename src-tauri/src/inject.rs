//! Remplace le mot en cours dans l'application active (frappes simulées).

use crate::hook::MAGIC;
use std::time::Duration;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

fn key(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: MAGIC },
        },
    }
}

fn send(inputs: &[INPUT]) {
    unsafe {
        SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

/// Efface `erase` caractères puis tape `text`.
/// Les lettres présentes sur le clavier sont tapées comme de vraies touches
/// (les applis récentes, Bloc-notes ou Word, mélangent parfois les caractères
/// Unicode simulés) ; le reste (ê, ï…) passe en Unicode.
pub fn replace(erase: usize, text: &str) {
    let back: Vec<INPUT> = (0..erase).flat_map(|_| [key(VK_BACK, 0, 0), key(VK_BACK, 0, KEYEVENTF_KEYUP)]).collect();
    send(&back);
    std::thread::sleep(Duration::from_millis(20));

    let (hkl, caps) = unsafe {
        let tid = GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
        (GetKeyboardLayout(tid), GetKeyState(VK_CAPITAL as i32) & 1 != 0)
    };
    for ch in text.chars() {
        let mut units = [0u16; 2];
        let units = ch.encode_utf16(&mut units);
        let scan = if units.len() == 1 && !caps { unsafe { VkKeyScanExW(units[0], hkl) } } else { -1 };
        let (vk, mods) = ((scan & 0xFF) as u16, (scan >> 8) & 0xFF);
        if scan != -1 && mods & !1 == 0 {
            let shift = mods & 1 != 0;
            let mut seq = Vec::with_capacity(4);
            if shift {
                seq.push(key(VK_SHIFT, 0, 0));
            }
            seq.push(key(vk, 0, 0));
            seq.push(key(vk, 0, KEYEVENTF_KEYUP));
            if shift {
                seq.push(key(VK_SHIFT, 0, KEYEVENTF_KEYUP));
            }
            send(&seq);
        } else {
            for u in units.iter() {
                send(&[key(0, *u, KEYEVENTF_UNICODE), key(0, *u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)]);
            }
        }
        std::thread::sleep(Duration::from_millis(3));
    }
}
