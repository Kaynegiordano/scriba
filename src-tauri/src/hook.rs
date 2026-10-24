//! Écoute globale du clavier et de la souris (hooks bas niveau Windows) pour
//! suivre la phrase en cours de frappe dans n'importe quelle application.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// Marque nos propres frappes simulées pour que le hook les ignore.
pub const MAGIC: usize = 0x5C21_BA00;
/// Marque les frappes de l'élève que Scriba rejoue après une correction.
const REPLAY: usize = 0x5C21_BA01;
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
    /// ouvrir la conjugaison (Ctrl+Alt+C)
    Conj,
    /// correction automatique du mot qui vient de se terminer
    Auto(AutoFix),
    /// l'élève annule la dernière correction automatique (Retour arrière)
    Undo(AutoFix),
    /// « ce mot est correct » (F8)
    Learn,
    /// lire à voix haute / ouvrir la fenêtre de lecture (Ctrl+Alt+R)
    Read,
    /// ouvrir la calculatrice (Ctrl+Alt+K)
    Calc,
    /// deux espaces de suite après un mot : « mot␠ » devient « mot. » (comme sur un smartphone)
    DoubleSpace,
}

#[derive(Clone)]
pub struct AutoFix {
    /// caractères à effacer (depuis le premier mot corrigé jusqu'au curseur)
    pub erase: usize,
    /// ce qu'on retape : la fin de phrase corrigée + l'espace ou la ponctuation
    pub text: String,
    /// ce que l'élève avait tapé (sans l'espace finale), pour annuler
    pub undo: String,
    /// (avant, après, type) pour chaque mot corrigé
    pub pairs: Vec<(String, String, &'static str)>,
    pub reason: String,
    /// la correction a été déclenchée par Entrée : la touche est rejouée après le mot corrigé
    pub enter: bool,
}

/// Une touche tapée pendant une correction, rejouée juste après.
#[derive(Clone, Copy)]
struct Ev {
    vk: u16,
    flags: u32,
    up: bool,
}

struct State {
    sentence: Vec<char>,
    dead: Option<char>,
    caps: bool,
    last_fg: isize,
    /// fenêtre où Scriba se tait (jeu en plein écran…)
    paused: bool,
    /// application d'écriture : la seule où la correction automatique agit
    writing: bool,
    /// la correction automatique a le droit de s'appliquer dans cette fenêtre
    corrects: bool,
    /// navigateur : un champ de mot de passe doit être vérifié avant de corriger
    browser: bool,
    /// instant de la dernière frappe d'espace (double espace = point)
    last_space: u64,
    /// dernière correction automatique, annulable par Retour arrière
    last_auto: Option<AutoFix>,
}

static STATE: Mutex<State> = Mutex::new(State {
    sentence: Vec::new(),
    dead: None,
    caps: false,
    last_fg: 0,
    paused: false,
    writing: false,
    corrects: false,
    browser: false,
    last_space: 0,
    last_auto: None,
});

/// Une correction est en cours : les frappes de l'élève attendent (file ci-dessous).
static BUSY: AtomicBool = AtomicBool::new(false);
/// Une correction attend d'être faite : même les frappes rejouées attendent.
static HOLD: AtomicBool = AtomicBool::new(false);
static BUSY_SINCE: AtomicU64 = AtomicU64::new(0);
static QUEUE: Mutex<Vec<Ev>> = Mutex::new(Vec::new());
static REPLAY_SEEN: AtomicU64 = AtomicU64::new(0);
/// Le mot signalé est inconnu : F8 peut l'ajouter au dictionnaire.
pub static LEARNABLE: AtomicBool = AtomicBool::new(false);

/// Applications d'écriture : jamais mises en veille, même en plein écran.
const WRITING_APPS: &[&str] = &[
    "winword.exe", "notepad.exe", "wordpad.exe", "msedge.exe", "chrome.exe", "brave.exe", "firefox.exe",
    "opera.exe", "powerpnt.exe", "excel.exe", "onenote.exe", "outlook.exe", "olk.exe", "soffice.bin",
    "swriter.exe", "soffice.exe", "teams.exe", "ms-teams.exe", "discord.exe", "whatsapp.exe", "telegram.exe",
    "slack.exe", "thunderbird.exe", "claude.exe", "obsidian.exe", "notion.exe", "signal.exe", "zoom.exe",
    "vivaldi.exe", "chatgpt.exe", "joplin.exe", "typora.exe",
];

/// Là où on ne corrige jamais seul : terminaux et éditeurs de code (une commande ou un programme n.est pas du français).
const NO_AUTO_APPS: &[&str] = &[
    "windowsterminal.exe", "wt.exe", "cmd.exe", "powershell.exe", "pwsh.exe", "conhost.exe", "openconsole.exe",
    "bash.exe", "wsl.exe", "mintty.exe", "putty.exe", "code.exe", "cursor.exe", "devenv.exe", "idea64.exe",
    "pycharm64.exe", "webstorm64.exe", "sublime_text.exe", "notepad++.exe",
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
        // une fenêtre normale agrandie (avec sa barre de titre) n'est pas un jeu, même sur un écran sans barre des tâches
        let captioned = (GetWindowLongW(hwnd, GWL_STYLE) as u32) & 0x00C0_0000 == 0x00C0_0000;
        fullscreen && !captioned && !WRITING_APPS.contains(&exe_name(hwnd).as_str())
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

fn now_ms() -> u64 {
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}

fn is_terminator(c: char) -> bool {
    matches!(c, ' ' | '\u{a0}' | ',' | '.' | ';' | ':' | '!' | '?' | ')' | ']' | '»' | '"' | '”' | '…')
}

/// Champ de mot de passe classique (les navigateurs ne se laissent pas interroger : on s'appuie
/// sur le fait que Scriba ne corrige jamais un mot collé à autre chose).
fn focus_is_password(fg: isize) -> bool {
    unsafe {
        let tid = GetWindowThreadProcessId(fg as HWND, std::ptr::null_mut());
        let mut gi: GUITHREADINFO = std::mem::zeroed();
        gi.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(tid, &mut gi) == 0 || gi.hwndFocus.is_null() {
            return false;
        }
        let mut class = [0u16; 16];
        let n = GetClassNameW(gi.hwndFocus, class.as_mut_ptr(), 16) as usize;
        String::from_utf16_lossy(&class[..n]).eq_ignore_ascii_case("edit")
            && (GetWindowLongW(gi.hwndFocus, GWL_STYLE) as u32) & 0x20 != 0 // ES_PASSWORD
    }
}

/// La correction sûre du mot qui se termine, s'il y en a une.
fn auto_candidate(st: &State, fg: isize, term: char) -> Option<AutoFix> {
    let mode = crate::custom::mode();
    if mode == crate::custom::OFF {
        return None;
    }
    let lex = crate::try_lex()?;
    // adresse, nom de fichier, champ de recherche d'un mot : il faut une vraie phrase
    if !matches!(term, ' ' | '\u{a0}') && !st.sentence.contains(&' ') {
        return None;
    }
    if focus_is_password(fg) {
        return None;
    }
    let fixes = crate::grammar::auto_fixes(lex, &st.sentence, term, mode);
    let start = fixes.first()?.start;
    let sentence = &st.sentence;
    let mut text = String::new();
    let mut pos = start;
    for c in &fixes {
        text.extend(&sentence[pos..c.start]);
        text.push_str(&c.replacement);
        pos = c.end;
    }
    text.extend(&sentence[pos..]);
    let enter = matches!(term, '\r' | '\n');
    if !enter {
        text.push(term);
    }
    Some(AutoFix {
        enter,
        erase: sentence.len() - start,
        text,
        undo: sentence[start..].iter().collect(),
        pairs: fixes.iter().map(|c| (c.original.clone(), c.replacement.clone(), c.kind)).collect(),
        reason: fixes.last().map(|c| c.reason.clone()).unwrap_or_default(),
    })
}

fn begin_hold() {
    BUSY_SINCE.store(now_ms(), Ordering::SeqCst);
    BUSY.store(true, Ordering::SeqCst);
    HOLD.store(true, Ordering::SeqCst);
}

/// Fin d'une correction : on rejoue, dans l'ordre, les touches tapées entre-temps.
pub fn finish_hold() {
    HOLD.store(false, Ordering::SeqCst);
    drain();
}

fn drain() {
    loop {
        let mut batch = std::mem::take(&mut *QUEUE.lock().unwrap());
        if batch.is_empty() {
            BUSY.store(false, Ordering::SeqCst);
            // une frappe a pu arriver entre-temps
            if QUEUE.lock().unwrap().is_empty() {
                return;
            }
            BUSY.store(true, Ordering::SeqCst);
            continue;
        }
        let mut done = 0;
        for ev in &batch {
            if HOLD.load(Ordering::SeqCst) {
                break; // une nouvelle correction a démarré : le reste attend son tour
            }
            replay_one(ev);
            done += 1;
        }
        if done < batch.len() {
            let rest: Vec<Ev> = batch.drain(done..).collect();
            QUEUE.lock().unwrap().splice(0..0, rest);
            return;
        }
    }
}

fn replay_one(ev: &Ev) {
    let flags = (if ev.up { KEYEVENTF_KEYUP } else { 0 }) | (if ev.flags & 1 != 0 { KEYEVENTF_EXTENDEDKEY } else { 0 });
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: ev.vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: REPLAY } },
    };
    let target = REPLAY_SEEN.load(Ordering::SeqCst) + 1;
    unsafe {
        SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
    }
    // on attend que le hook l'ait vue : elle peut déclencher une nouvelle correction
    let start = now_ms();
    while REPLAY_SEEN.load(Ordering::SeqCst) < target && now_ms() < start + 40 {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    // comme pour les mots retapes : trop vite, certaines applis perdent des touches
    if !ev.up {
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
}

/// L'application où l'élève écrit est une appli d'écriture (on peut y simuler Ctrl+C sans risque :
/// dans un terminal, Ctrl+C arrêterait le programme en cours).
pub fn target_is_writing_app() -> bool {
    STATE.lock().unwrap().writing
}

/// La fenêtre où l'on écrit est un navigateur.
pub fn target_is_browser() -> bool {
    STATE.lock().unwrap().browser
}

pub fn current_sentence() -> String {
    STATE.lock().unwrap().sentence.iter().collect()
}

pub fn set_last_auto(a: Option<AutoFix>) {
    STATE.lock().unwrap().last_auto = a;
}

fn publish(st: &State) {
    send(Msg::Text(st.sentence.iter().collect()));
}

/// La barre réapparaît : on renvoie la phrase suivie pour recalculer ses propositions.
pub fn republish() {
    publish(&STATE.lock().unwrap());
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
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MsgWaitForMultipleObjects, PeekMessageW, UnhookWindowsHookEx, PM_REMOVE, QS_ALLINPUT, WM_QUIT,
        };
        let module = GetModuleHandleW(std::ptr::null());
        let install = || (
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0),
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0),
        );
        let (mut kb, mut mouse) = install();
        let mut last = std::time::Instant::now();
        let mut msg: MSG = std::mem::zeroed();
        loop {
            MsgWaitForMultipleObjects(0, std::ptr::null(), 0, 5000, QS_ALLINPUT);
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    return;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            // Windows retire sans prévenir un branchement clavier jugé trop lent (ou après une veille) :
            // on le remet régulièrement, sans rien casser s'il était encore en place.
            if last.elapsed() >= std::time::Duration::from_secs(15) && !BUSY.load(Ordering::Relaxed) && !HOLD.load(Ordering::Relaxed) {
                UnhookWindowsHookEx(kb);
                UnhookWindowsHookEx(mouse);
                (kb, mouse) = install();
                last = std::time::Instant::now();
            }
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
            let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
            let replayed = k.dwExtraInfo == REPLAY;

            // Une correction est en cours : la frappe attend son tour, rien n'est perdu.
            let waiting = if replayed { HOLD.load(Ordering::SeqCst) } else { BUSY.load(Ordering::SeqCst) };
            if waiting {
                QUEUE.lock().unwrap().push(Ev { vk: k.vkCode as u16, flags: k.flags, up });
                if replayed {
                    REPLAY_SEEN.fetch_add(1, Ordering::SeqCst);
                } else if now_ms() > BUSY_SINCE.load(Ordering::SeqCst) + 1500 {
                    // sécurité : le clavier ne reste jamais bloqué
                    HOLD.store(false, Ordering::SeqCst);
                    std::thread::spawn(drain);
                }
                return 1;
            }

            let swallow = if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                let s = on_key_down(k);
                if s {
                    SWALLOW_UP.store(k.vkCode, Ordering::Relaxed);
                }
                s
            } else if up && SWALLOW_UP.load(Ordering::Relaxed) == k.vkCode {
                SWALLOW_UP.store(0, Ordering::Relaxed);
                true
            } else {
                false
            };
            if replayed {
                REPLAY_SEEN.fetch_add(1, Ordering::SeqCst);
            }
            if swallow {
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

    // Ctrl + Alt gauche + L : afficher / masquer ; + C : conjugaison (AltGr exclu)
    let left_alt_combo = ctrl && pressed(VK_LMENU) && !pressed(VK_RMENU);
    if left_alt_combo && vk == b'L' as u16 {
        send(Msg::Toggle);
        crate::inject::mask_alt();
        return true;
    }
    if left_alt_combo && vk == b'C' as u16 {
        send(Msg::Conj);
        crate::inject::mask_alt();
        return true;
    }
    if left_alt_combo && vk == b'R' as u16 {
        send(Msg::Read);
        crate::inject::mask_alt();
        return true;
    }
    if left_alt_combo && vk == b'K' as u16 {
        send(Msg::Calc);
        crate::inject::mask_alt();
        return true;
    }

    // Fenêtres de Scriba (barre, conjugaison) : on n'y suit pas la frappe
    let fg = GetForegroundWindow() as isize;
    let mut pid = 0u32;
    GetWindowThreadProcessId(fg as HWND, &mut pid);
    if pid == std::process::id() {
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
        let exe = exe_name(fg as HWND);
        st.writing = WRITING_APPS.contains(&exe.as_str());
        // une fenêtre dont on ne peut pas lire le nom est souvent lancée en administrateur : la frappe n'y passe pas
        st.browser = crate::uia::BROWSERS.contains(&exe.as_str());
        st.corrects = st.writing || (!exe.is_empty() && !NO_AUTO_APPS.contains(&exe.as_str()));
        st.last_auto = None;
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
    // F8 : ce mot est correct, on l'ajoute au dictionnaire
    if vk == VK_F8 && visible && no_mods && LEARNABLE.load(Ordering::Relaxed) {
        send(Msg::Learn);
        return true;
    }

    match vk {
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU
        | VK_RMENU | VK_LWIN | VK_RWIN => return false,
        VK_BACK => {
            if no_mods {
                if let Some(a) = st.last_auto.take() {
                    begin_hold();
                    send(Msg::Undo(a));
                    return true;
                }
            }
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
    st.last_auto = None;

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
    if ch == Some(' ') {
        let now = now_ms();
        let quick = now.saturating_sub(st.last_space) < 700;
        st.last_space = now;
        let n = st.sentence.len();
        let after_word = n >= 2 && st.sentence[n - 1] == ' ' && st.sentence[n - 2].is_alphanumeric();
        // deux espaces de suite après un mot : un point (jamais dans un terminal, un éditeur de code ou un jeu)
        if quick && after_word && st.corrects && no_mods && crate::custom::double_space() {
            begin_hold();
            send(Msg::DoubleSpace);
            return true;
        }
    }
    if let Some(c) = ch {
        // Entrée termine aussi le mot (on valide un message sans avoir tapé d'espace) ; la touche est rejouée
        // juste après la correction, sauf si c'est déjà elle que l'on rejoue
        let enter = matches!(c, '\r' | '\n') && vk == VK_RETURN && k.dwExtraInfo != REPLAY;
        if (is_terminator(c) || enter) && st.corrects && st.sentence.last().is_some_and(|x| x.is_alphabetic()) {
            if let Some(fix) = auto_candidate(&st, fg, c) {
                if fix.enter {
                    let mut q = QUEUE.lock().unwrap();
                    q.push(Ev { vk: k.vkCode as u16, flags: k.flags, up: false });
                    q.push(Ev { vk: k.vkCode as u16, flags: k.flags, up: true });
                }
                begin_hold();
                send(Msg::Auto(fix));
                return true; // l'espace (ou la ponctuation) est retapée avec le mot corrigé
            }
        }
    }
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
                st.last_auto = None;
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
