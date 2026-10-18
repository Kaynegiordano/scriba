//! Champ de mot de passe d'un navigateur : les navigateurs ne se laissent pas interroger comme une
//! fenêtre Windows classique, il faut passer par UI Automation. Appelé seulement quand Scriba
//! s'apprête à corriger un mot (jamais à chaque frappe), depuis le fil de travail, pas depuis le clavier.

use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};

/// Navigateurs dont les champs sont décrits par UI Automation (les autres fenêtres : `focus_is_password`).
pub const BROWSERS: &[&str] = &["msedge.exe", "chrome.exe", "brave.exe", "firefox.exe", "opera.exe", "vivaldi.exe"];

thread_local! {
    static AUTOMATION: Option<IUIAutomation> = unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()
    };
}

/// Vrai si l'élément qui a le focus est un champ de mot de passe.
pub fn focused_is_password() -> bool {
    AUTOMATION.with(|a| {
        let Some(a) = a else { return false };
        unsafe {
            a.GetFocusedElement()
                .and_then(|e| e.CurrentIsPassword())
                .map(|b| b.as_bool())
                .unwrap_or(false)
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn interroge_sans_planter() {
        // sur le bureau de test, aucun champ de mot de passe n'a le focus
        assert!(!super::focused_is_password());
    }
}
