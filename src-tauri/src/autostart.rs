//! Démarrage avec Windows : une valeur dans HKCU\...\Run (aucun droit d'administrateur).

use std::ptr::{null, null_mut};
use windows_sys::Win32::System::Registry::*;

const KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const NAME: &str = "Scriba";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn enabled() -> bool {
    enabled_as(NAME)
}

fn enabled_as(name: &str) -> bool {
    unsafe {
        let mut h: HKEY = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(KEY).as_ptr(), 0, KEY_READ, &mut h) != 0 {
            return false;
        }
        let found = RegQueryValueExW(h, wide(name).as_ptr(), null(), null_mut(), null_mut(), null_mut()) == 0;
        RegCloseKey(h);
        found
    }
}

/// Active ou retire le démarrage automatique (Scriba se lance alors avec l'exécutable actuel).
pub fn set(on: bool) {
    set_as(NAME, on);
}

fn set_as(name: &str, on: bool) {
    unsafe {
        let mut h: HKEY = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(KEY).as_ptr(), 0, KEY_SET_VALUE, &mut h) != 0 {
            return;
        }
        if on {
            if let Ok(exe) = std::env::current_exe() {
                let value = wide(&format!("\"{}\"", exe.display()));
                RegSetValueExW(h, wide(name).as_ptr(), 0, REG_SZ, value.as_ptr() as *const u8, (value.len() * 2) as u32);
            }
        } else {
            RegDeleteValueW(h, wide(name).as_ptr());
        }
        RegCloseKey(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demarrage_automatique() {
        // une valeur de test, retirée aussitôt : la vraie valeur « Scriba » n'est pas touchée
        let name = "ScribaTestAutostart";
        set_as(name, false);
        assert!(!enabled_as(name));
        set_as(name, true);
        assert!(enabled_as(name));
        set_as(name, false);
        assert!(!enabled_as(name));
    }
}
