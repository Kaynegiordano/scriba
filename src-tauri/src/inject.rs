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

const GAP_MS: u64 = 8;

/// Après un raccourci Ctrl+Alt+… avalé par Scriba : une touche neutre pour que
/// le relâchement d'Alt n'ouvre pas le menu de l'application.
pub fn mask_alt() {
    const VK_NEUTRAL: VIRTUAL_KEY = 0xE8; // code non attribué
    send(&[key(VK_NEUTRAL, 0, 0), key(VK_NEUTRAL, 0, KEYEVENTF_KEYUP)]);
}

fn send(inputs: &[INPUT]) {
    unsafe {
        SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

/// Une touche (code virtuel seul : avec le code matériel, le Bloc-notes récent
/// mélange les lettres).
fn press(vk: VIRTUAL_KEY, _hkl: HKL) -> [INPUT; 2] {
    [key(vk, 0, 0), key(vk, 0, KEYEVENTF_KEYUP)]
}

/// Maj / Ctrl / Alt encore enfoncés (le point se tape avec Maj sur AZERTY) fausseraient
/// ce qu'on tape : on les relâche virtuellement. Le vrai relâchement, mis en attente
/// par le hook, est rejoué ensuite sans dommage.
fn release_modifiers() {
    let held: Vec<INPUT> = [VK_LSHIFT, VK_RSHIFT, VK_LCONTROL, VK_RCONTROL, VK_LMENU, VK_RMENU]
        .into_iter()
        .filter(|vk| unsafe { (GetAsyncKeyState(*vk as i32) as u16) & 0x8000 != 0 })
        .map(|vk| key(vk, 0, KEYEVENTF_KEYUP))
        .collect();
    if !held.is_empty() {
        send(&held);
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Ctrl+C dans l'application active (pour lire le texte sélectionné).
pub fn copy_selection() {
    release_modifiers();
    send(&[
        key(VK_CONTROL, 0, 0),
        key(b'C' as u16, 0, 0),
        key(b'C' as u16, 0, KEYEVENTF_KEYUP),
        key(VK_CONTROL, 0, KEYEVENTF_KEYUP),
    ]);
}

/// Efface `erase` caractères puis tape `text`.
/// Les lettres présentes sur le clavier sont tapées comme de vraies touches
/// (les applis récentes, Bloc-notes ou Word, mélangent parfois les caractères
/// Unicode simulés) ; le reste (ê, ï…) passe en Unicode.
pub fn replace(erase: usize, text: &str) {
    release_modifiers();
    let (hkl, caps) = unsafe {
        let tid = GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
        (GetKeyboardLayout(tid), GetKeyState(VK_CAPITAL as i32) & 1 != 0)
    };
    // Une touche à la fois, espacées : envoyées d'un bloc, les applis occupées
    // (Bloc-notes avec correcteur, Word, navigateurs) en perdent une partie.
    let gap = Duration::from_millis(std::env::var("SCRIBA_GAP_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(GAP_MS));
    for _ in 0..erase {
        send(&press(VK_BACK, hkl));
        std::thread::sleep(gap);
    }
    std::thread::sleep(gap * 3);

    for ch in text.chars() {
        let mut units = [0u16; 2];
        let units = ch.encode_utf16(&mut units);
        let scan = if units.len() == 1 && !caps { unsafe { VkKeyScanExW(units[0], hkl) } } else { -1 };
        let (vk, mods) = ((scan & 0xFF) as u16, (scan >> 8) & 0xFF);
        if scan != -1 && mods & !1 == 0 {
            let shift = mods & 1 != 0;
            let [shift_down, shift_up] = press(VK_SHIFT, hkl);
            let mut seq = Vec::with_capacity(4);
            if shift {
                seq.push(shift_down);
            }
            seq.extend(press(vk, hkl));
            if shift {
                seq.push(shift_up);
            }
            send(&seq);
        } else {
            for u in units.iter() {
                send(&[key(0, *u, KEYEVENTF_UNICODE), key(0, *u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)]);
            }
        }
        std::thread::sleep(gap);
    }
}
