//! Écoute globale du clavier et de la souris (hooks bas niveau Windows) pour
//! suivre la phrase en cours de frappe dans n'importe quelle application.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// Marque nos propres frappes simulées pour que le hook les ignore.
pub const MAGIC: usize = 0x5C21_BA00;
/// Au-delà, on oublie le début de la phrase.
const MAX_CHARS: usize = 240;

pub enum Msg {
    /// la phrase telle que tapée depuis le dernier clic / retour à la ligne
    Text(String),
    Pick(usize),
    /// appliquer la correction proposée (F7)
    Fix,
    /// ponctuation tapée juste après une espace ajoutée par Scriba : « maison , » -> « maison, »
    Punct(char),
    Toggle,
}

struct State {
    sentence: Vec<char>,
    dead: Option<char>,
    caps: bool,
    last_fg: isize,
    /// fenêtre où Scriba se tait (jeu en plein écran…)
    paused: bool,
}

static STATE: Mutex<State> =
    Mutex::new(State { sentence: Vec::new(), dead: None, caps: false, last_fg: 0, paused: false });

/// Applications d'écriture : jamais mises en veille, même en plein écran.
const WRITING_APPS: &[&str] = &[
    "winword.exe", "notepad.exe", "wordpad.exe", "msedge.exe", "chrome.exe", "brave.exe", "firefox.exe",
    "opera.exe", "powerpnt.exe", "excel.exe", "onenote.exe", "outlook.exe", "olk.exe", "soffice.bin",
    "swriter.exe", "teams.exe", "ms-teams.exe",
];

fn exe_name(hwnd: HWND) -> String {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return String::new();
        }
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len) != 0;
        CloseHandle(h);
        if !ok {
            return String::new();
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().unwrap_or("").to_lowercase()
    }
}

/// Une fenêtre qui couvre tout l'écran et n'est pas une appli d'écriture : sûrement un jeu.
fn looks_like_game(hwnd: HWND) -> bool {
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut r) == 0 {
            return false;
        }
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut mi) == 0 {
            return false;
        }
        let m = mi.rcMonitor;
        let fullscreen = r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom;
        fullscreen && !WRITING_APPS.contains(&exe_name(hwnd).as_str())
    }
}
static TX: OnceLock<Sender<Msg>> = OnceLock::new();
pub static VISIBLE: AtomicBool = AtomicBool::new(true);
/// Une correction est affichée (F7 l'applique).
pub static CORRECTION: AtomicBool = AtomicBool::new(false);
/// Scriba vient d'ajouter une espace après un mot choisi.
static AUTO_SPACE: AtomicBool = AtomicBool::new(false);
pub static OWN_HWND: AtomicIsize = AtomicIsize::new(0);
/// Touche dont on doit aussi avaler le relâchement.
static SWALLOW_UP: AtomicU32 = AtomicU32::new(0);

fn send(m: Msg) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(m);
    }
}

fn publish(st: &State) {
    send(Msg::Text(st.sentence.iter().collect()));
}

fn clear(st: &mut State) {
    st.sentence.clear();
    st.dead = None;
}

fn current_word(st: &State) -> usize {
    st.sentence.iter().rev().take_while(|c| c.is_alphabetic()).count()
}

/// Après une frappe simulée par Scriba : retire `erase` caractères et ajoute `text`.
pub fn set_tail(erase: usize, text: &str, auto_space: bool) {
    let mut st = STATE.lock().unwrap();
    let keep = st.sentence.len().saturating_sub(erase);
    st.sentence.truncate(keep);
    st.sentence.extend(text.chars());
    st.dead = None;
    AUTO_SPACE.store(auto_space, Ordering::Relaxed);
}

/// Dernière fenêtre (autre que Scriba) où l'élève écrivait.
pub fn last_target() -> isize {
    STATE.lock().unwrap().last_fg
}

pub fn start(tx: Sender<Msg>) {
    let _ = TX.set(tx);
    std::thread::spawn(|| unsafe {
        STATE.lock().unwrap().caps = GetKeyState(VK_CAPITAL as i32) & 1 != 0;
        let module = GetModuleHandleW(std::ptr::null());
        SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0);
        SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0);
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

fn pressed(vk: VIRTUAL_KEY) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16) & 0x8000 != 0 }
}

fn compose(dead: char, c: char) -> Option<char> {
    let table: &[(char, &str, &str)] = &[
        ('^', "aeiouAEIOU", "âêîôûÂÊÎÔÛ"),
        ('¨', "aeiouyAEIOU", "äëïöüÿÄËÏÖÜ"),
        ('`', "aeiouAEIOU", "àèìòùÀÈÌÒÙ"),
        ('´', "aeiouAEIOU", "áéíóúÁÉÍÓÚ"),
        ('~', "anoANO", "ãñõÃÑÕ"),
    ];
    let (_, from, to) = table.iter().find(|(d, _, _)| *d == dead)?;
    let pos = from.chars().position(|x| x == c)?;
    to.chars().nth(pos)
}

unsafe extern "system" fn keyboard_proc(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let k = &*(lp as *const KBDLLHOOKSTRUCT);
        if k.dwExtraInfo != MAGIC {
            let msg = wp as u32;
            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                if on_key_down(k) {
                    SWALLOW_UP.store(k.vkCode, Ordering::Relaxed);
                    return 1;
                }
            } else if (msg == WM_KEYUP || msg == WM_SYSKEYUP) && SWALLOW_UP.load(Ordering::Relaxed) == k.vkCode {
                SWALLOW_UP.store(0, Ordering::Relaxed);
                return 1;
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wp, lp)
}

/// Renvoie vrai si la touche doit être avalée (raccourcis de Scriba).
unsafe fn on_key_down(k: &KBDLLHOOKSTRUCT) -> bool {
    let vk = k.vkCode as VIRTUAL_KEY;
    let ctrl = pressed(VK_CONTROL);
    let alt = pressed(VK_MENU);
    let shift = pressed(VK_SHIFT);

    // Ctrl + Alt gauche + L : afficher / masquer (AltGr exclu)
    if vk == b'L' as u16 && ctrl && pressed(VK_LMENU) && !pressed(VK_RMENU) {
        send(Msg::Toggle);
        return true;
    }

    let fg = GetForegroundWindow() as isize;
    let own = OWN_HWND.load(Ordering::Relaxed);
    if own != 0 && fg == own {
        return false;
    }

    let mut st = STATE.lock().unwrap();
    if vk == VK_CAPITAL {
        st.caps = !st.caps;
        return false;
    }
    if fg != st.last_fg {
        st.last_fg = fg;
        st.paused = looks_like_game(fg as HWND);
        clear(&mut st);
        publish(&st);
    }
    if st.paused {
        return false;
    }

    let visible = VISIBLE.load(Ordering::Relaxed);
    let no_mods = !ctrl && !alt && !shift;
    // F1..F6 : choisir une proposition
    if (VK_F1..=VK_F6).contains(&vk) && visible && no_mods && current_word(&st) > 0 {
        send(Msg::Pick((vk - VK_F1) as usize));
        return true;
    }
    // F7 : appliquer la correction
    if vk == VK_F7 && visible && no_mods && CORRECTION.load(Ordering::Relaxed) {
        send(Msg::Fix);
        return true;
    }

    match vk {
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU
        | VK_RMENU | VK_LWIN | VK_RWIN => return false,
        VK_BACK => {
            if ctrl {
                clear(&mut st);
            } else {
                st.sentence.pop();
            }
            st.dead = None;
            AUTO_SPACE.store(false, Ordering::Relaxed);
            publish(&st);
            return false;
        }
        _ => {}
    }
    let auto_space = AUTO_SPACE.swap(false, Ordering::Relaxed);

    // Raccourcis (Ctrl+…, Alt+…) : on perd le fil de la phrase
    if ctrl != alt {
        clear(&mut st);
        publish(&st);
        return false;
    }

    let ch = if vk == VK_PACKET {
        char::from_u32(k.scanCode)
    } else {
        let mut ks = [0u8; 256];
        if shift {
            ks[VK_SHIFT as usize] = 0x80;
        }
        if ctrl {
            ks[VK_CONTROL as usize] = 0x80;
        }
        if alt {
            ks[VK_MENU as usize] = 0x80;
        }
        if st.caps {
            ks[VK_CAPITAL as usize] = 0x01;
        }
        let tid = GetWindowThreadProcessId(fg as HWND, std::ptr::null_mut());
        let hkl = GetKeyboardLayout(tid);
        let mut out = [0u16; 8];
        // drapeau 0x4 : ne pas modifier l'état clavier (préserve les touches mortes ^ ¨)
        let n = ToUnicodeEx(k.vkCode, k.scanCode, ks.as_ptr(), out.as_mut_ptr(), 8, 0x4, hkl);
        if n < 0 {
            st.dead = char::from_u32(out[0] as u32);
            return false;
        }
        if n == 0 {
            None
        } else {
            String::from_utf16_lossy(&out[..n as usize]).chars().next()
        }
    };

    let dead = st.dead.take();
    match ch {
        Some(c) if c.is_alphabetic() => {
            st.sentence.push(dead.and_then(|d| compose(d, c)).unwrap_or(c));
        }
        // ponctuation collée au mot choisi : on retire l'espace ajoutée par Scriba
        Some(c @ (',' | '.' | ')')) if auto_space && st.sentence.last() == Some(&' ') => {
            send(Msg::Punct(c));
            return true;
        }
        // retour à la ligne, tabulation, échap, flèches… : nouvelle phrase
        Some(c) if c.is_control() => clear(&mut st),
        Some(c) => st.sentence.push(c),
        None => clear(&mut st),
    }
    if st.sentence.len() > MAX_CHARS {
        let cut = st.sentence.iter().skip(MAX_CHARS / 2).position(|c| *c == ' ').map(|p| p + MAX_CHARS / 2 + 1);
        let cut = cut.unwrap_or(st.sentence.len());
        st.sentence.drain(..cut);
    }
    publish(&st);
    false
}

unsafe extern "system" fn mouse_proc(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let msg = wp as u32;
    if code == HC_ACTION as i32 && (msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN) {
        let m = &*(lp as *const MSLLHOOKSTRUCT);
        if m.dwExtraInfo != MAGIC {
            let own = OWN_HWND.load(Ordering::Relaxed) as HWND;
            let mut r: RECT = std::mem::zeroed();
            let on_bar = VISIBLE.load(Ordering::Relaxed)
                && GetWindowRect(own, &mut r) != 0
                && m.pt.x >= r.left
                && m.pt.x < r.right
                && m.pt.y >= r.top
                && m.pt.y < r.bottom;
            if !on_bar {
                // un clic déplace le curseur : la phrase suivie n'est plus fiable
                let mut st = STATE.lock().unwrap();
                AUTO_SPACE.store(false, Ordering::Relaxed);
                if !st.sentence.is_empty() {
                    clear(&mut st);
                    publish(&st);
                }
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wp, lp)
}
