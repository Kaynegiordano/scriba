//! Écoute globale du clavier et de la souris (hooks bas niveau Windows) pour
//! suivre le mot en cours de frappe dans n'importe quelle application.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// Marque nos propres frappes simulées pour que le hook les ignore.
pub const MAGIC: usize = 0x5C21_BA00;

pub enum Msg {
    Word(String),
    Pick(usize),
    Toggle,
}

struct State {
    buf: Vec<char>,
    dead: Option<char>,
    caps: bool,
    last_fg: isize,
}

static STATE: Mutex<State> = Mutex::new(State { buf: Vec::new(), dead: None, caps: false, last_fg: 0 });
static TX: OnceLock<Sender<Msg>> = OnceLock::new();
pub static VISIBLE: AtomicBool = AtomicBool::new(true);
pub static OWN_HWND: AtomicIsize = AtomicIsize::new(0);
/// Touche dont on doit aussi avaler le relâchement.
static SWALLOW_UP: AtomicU32 = AtomicU32::new(0);

fn send(m: Msg) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(m);
    }
}

fn publish(st: &State) {
    send(Msg::Word(st.buf.iter().collect()));
}

pub fn reset() {
    let mut st = STATE.lock().unwrap();
    st.buf.clear();
    st.dead = None;
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
        st.buf.clear();
        st.dead = None;
        publish(&st);
    }

    // F1..F6 : choisir une proposition
    if (VK_F1..=VK_F6).contains(&vk)
        && VISIBLE.load(Ordering::Relaxed)
        && !st.buf.is_empty()
        && !ctrl
        && !alt
        && !shift
    {
        send(Msg::Pick((vk - VK_F1) as usize));
        return true;
    }

    match vk {
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU
        | VK_RMENU | VK_LWIN | VK_RWIN => return false,
        VK_BACK => {
            if ctrl {
                st.buf.clear();
            } else {
                st.buf.pop();
            }
            st.dead = None;
            publish(&st);
            return false;
        }
        _ => {}
    }

    // Raccourcis (Ctrl+…, Alt+…) : on perd le fil du mot
    if ctrl != alt {
        st.buf.clear();
        st.dead = None;
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
            let c = dead.and_then(|d| compose(d, c)).unwrap_or(c);
            st.buf.push(c);
            if st.buf.len() > 40 {
                st.buf.clear();
            }
        }
        _ => st.buf.clear(),
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
                // un clic déplace le curseur : le mot en cours n'est plus fiable
                let mut st = STATE.lock().unwrap();
                if !st.buf.is_empty() {
                    st.buf.clear();
                    st.dead = None;
                    publish(&st);
                }
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wp, lp)
}
