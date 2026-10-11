#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod grammar;
mod hook;
mod inject;
mod lexicon;
mod phonetics;

use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use grammar::Correction;
use hook::Msg;
use lexicon::{Lexicon, Suggestion};
use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};
use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const MAX_SUGGESTIONS: usize = 6;
/// On ne corrige pas un mot trop loin derrière le curseur (il faudrait tout retaper).
const MAX_FIX_DISTANCE: usize = 80;

struct Tx(Sender<Msg>);

#[derive(Clone, Serialize)]
struct Fix {
    from: String,
    to: String,
    reason: String,
    kind: &'static str,
}

#[derive(Clone, Serialize)]
struct Payload {
    typed: String,
    items: Vec<Suggestion>,
    fix: Option<Fix>,
}

#[tauri::command]
fn pick(index: usize, tx: State<Tx>) {
    let _ = tx.0.send(Msg::Pick(index));
}

#[tauri::command]
fn apply_fix(tx: State<Tx>) {
    let _ = tx.0.send(Msg::Fix);
}

#[tauri::command]
fn toggle(tx: State<Tx>) {
    let _ = tx.0.send(Msg::Toggle);
}

/// Respecte la casse tapée : « Mezon » -> « Maison », « MEZON » -> « MAISON ».
fn match_case(typed: &str, word: &str) -> String {
    let n = typed.chars().count();
    if n > 1 && typed.chars().all(|c| !c.is_lowercase()) {
        return word.to_uppercase();
    }
    if typed.chars().next().is_some_and(|c| c.is_uppercase()) {
        let mut cs = word.chars();
        if let Some(f) = cs.next() {
            return f.to_uppercase().chain(cs).collect();
        }
    }
    word.to_string()
}

fn own_hwnd() -> HWND {
    hook::OWN_HWND.load(Ordering::Relaxed) as HWND
}

fn set_visible(show: bool) {
    hook::VISIBLE.store(show, Ordering::Relaxed);
    unsafe {
        ShowWindow(own_hwnd(), if show { SW_SHOWNOACTIVATE } else { SW_HIDE });
    }
}

/// Clic sur la barre : rendre la main à la fenêtre où l'élève écrivait.
fn focus_target() {
    unsafe {
        if GetForegroundWindow() == own_hwnd() {
            SetForegroundWindow(hook::last_target() as HWND);
            std::thread::sleep(Duration::from_millis(40));
        }
    }
}

/// Retape la fin de la phrase : efface `erase` caractères, tape `text`.
fn retype(erase: usize, text: &str, auto_space: bool) {
    hook::set_tail(erase, text, auto_space);
    focus_target();
    inject::replace(erase, text);
}

fn worker(app: AppHandle, rx: Receiver<Msg>) {
    let lex = Lexicon::load(include_str!("../data/lexique.tsv"));
    let mut sentence: Vec<char> = Vec::new();
    let mut typed = String::new();
    let mut items: Vec<Suggestion> = Vec::new();
    let mut fix: Option<Correction> = None;

    let refresh = |sentence: &[char], typed: &mut String, items: &mut Vec<Suggestion>, fix: &mut Option<Correction>| {
        let word_len = sentence.iter().rev().take_while(|c| c.is_alphabetic()).count();
        *typed = sentence[sentence.len() - word_len..].iter().collect();

        let toks = grammar::tokenize(&lex, sentence);
        // le mot en cours est le dernier jeton s'il touche la fin de la phrase
        let cur = toks.len().saturating_sub(if word_len > 0 { 1 } else { 0 });
        *fix = grammar::check(&lex, sentence)
            .into_iter()
            .rev()
            .find(|c| sentence.len() - c.start <= MAX_FIX_DISTANCE);
        let max = if fix.is_some() { MAX_SUGGESTIONS - 1 } else { MAX_SUGGESTIONS };
        *items = if typed.is_empty() {
            Vec::new()
        } else {
            let h = grammar::hint(&lex, &toks, cur.min(toks.len().saturating_sub(1)));
            lex.suggest(typed, &h, max)
        };
        for s in items.iter_mut() {
            s.word = match_case(typed, &s.word);
        }
        hook::CORRECTION.store(fix.is_some(), Ordering::Relaxed);
        let view = fix.as_ref().map(|c| Fix {
            from: c.original.clone(),
            to: c.replacement.clone(),
            reason: c.reason.clone(),
            kind: c.kind,
        });
        let _ = app.emit("suggestions", Payload { typed: typed.clone(), items: items.clone(), fix: view });
    };

    while let Ok(mut msg) = rx.recv() {
        // Ne garder que la dernière phrase si plusieurs frappes sont en attente
        if let Msg::Text(_) = msg {
            while let Ok(next) = rx.try_recv() {
                let is_text = matches!(next, Msg::Text(_));
                msg = next;
                if !is_text {
                    break;
                }
            }
        }
        match msg {
            Msg::Text(s) => {
                let s: Vec<char> = s.chars().collect();
                if s == sentence {
                    continue;
                }
                sentence = s;
                refresh(&sentence, &mut typed, &mut items, &mut fix);
            }
            Msg::Pick(i) => {
                let Some(s) = items.get(i) else { continue };
                let erase = typed.chars().count();
                let text = format!("{} ", s.word);
                retype(erase, &text, true);
                let keep = sentence.len() - erase;
                sentence.truncate(keep);
                sentence.extend(text.chars());
                refresh(&sentence, &mut typed, &mut items, &mut fix);
            }
            Msg::Fix => {
                let Some(c) = fix.take() else { continue };
                let erase = sentence.len() - c.start;
                let rest: String = sentence[c.end..].iter().collect();
                let text = format!("{}{rest}", c.replacement);
                retype(erase, &text, false);
                sentence.truncate(c.start);
                sentence.extend(text.chars());
                refresh(&sentence, &mut typed, &mut items, &mut fix);
            }
            Msg::Punct(p) => {
                // « maison , » -> « maison, »
                let text = p.to_string();
                retype(1, &text, false);
                sentence.pop();
                sentence.push(p);
                refresh(&sentence, &mut typed, &mut items, &mut fix);
            }
            Msg::Toggle => set_visible(!hook::VISIBLE.load(Ordering::Relaxed)),
        }
    }
}

fn main() {
    // Une seule instance (deux hooks clavier se marcheraient dessus)
    let name: Vec<u16> = "Local\\Scriba.Instance\0".encode_utf16().collect();
    unsafe {
        CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }
    }

    let (tx, rx) = channel::<Msg>();
    let hook_tx = tx.clone();
    let tray_tx = tx.clone();

    tauri::Builder::default()
        .manage(Tx(tx))
        .invoke_handler(tauri::generate_handler![pick, apply_fix, toggle])
        .setup(move |app| {
            let win = app.get_webview_window("main").expect("fenêtre principale");
            let hwnd = win.hwnd()?.0 as isize;
            hook::OWN_HWND.store(hwnd, Ordering::Relaxed);
            unsafe {
                // La barre ne doit jamais voler le focus de l'application où l'on écrit
                let h = hwnd as HWND;
                let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
                SetWindowLongPtrW(h, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE as isize);
                // En haut, au centre de l'écran principal
                let mut r = std::mem::zeroed();
                GetWindowRect(h, &mut r);
                let scale = win.scale_factor().unwrap_or(1.0);
                let x = (GetSystemMetrics(SM_CXSCREEN) - (r.right - r.left)) / 2;
                let y = (14.0 * scale) as i32;
                SetWindowPos(h, HWND_TOPMOST, x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED);
            }
            set_visible(true);

            let toggle_item = MenuItem::with_id(app, "toggle", "Afficher / masquer   Ctrl+Alt+L", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quitter Scriba", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&toggle_item, &quit_item])?;
            let menu_tx = tray_tx.clone();
            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().expect("icône"))
                .tooltip("Scriba")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, e| match e.id.as_ref() {
                    "toggle" => {
                        let _ = menu_tx.send(Msg::Toggle);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(move |_, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        let _ = tray_tx.send(Msg::Toggle);
                    }
                })
                .build(app)?;

            let handle = app.handle().clone();
            std::thread::spawn(move || worker(handle, rx));
            hook::start(hook_tx);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erreur au lancement de Scriba");
}
