#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hook;
mod inject;
mod lexicon;
mod phonetics;

use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

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

struct Tx(Sender<Msg>);

#[derive(Clone, Serialize)]
struct Payload {
    typed: String,
    items: Vec<Suggestion>,
}

#[tauri::command]
fn pick(index: usize, tx: State<Tx>) {
    let _ = tx.0.send(Msg::Pick(index));
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

fn worker(app: AppHandle, rx: Receiver<Msg>) {
    let lex = Lexicon::load(include_str!("../data/lexique.tsv"));
    let mut typed = String::new();
    let mut items: Vec<Suggestion> = Vec::new();
    let emit = |typed: &str, items: &[Suggestion]| {
        let _ = app.emit("suggestions", Payload { typed: typed.to_string(), items: items.to_vec() });
    };

    while let Ok(mut msg) = rx.recv() {
        // Ne garder que le dernier mot si plusieurs frappes sont en attente
        if let Msg::Word(_) = msg {
            while let Ok(next) = rx.try_recv() {
                let is_word = matches!(next, Msg::Word(_));
                msg = next;
                if !is_word {
                    break;
                }
            }
        }
        match msg {
            Msg::Word(w) => {
                if w == typed {
                    continue;
                }
                typed = w;
                items = lex.suggest(&typed, MAX_SUGGESTIONS);
                for s in &mut items {
                    s.word = match_case(&typed, &s.word);
                }
                emit(&typed, &items);
            }
            Msg::Pick(i) => {
                let Some(s) = items.get(i) else { continue };
                let erase = typed.chars().count();
                let text = format!("{} ", s.word);
                hook::reset();
                unsafe {
                    // Clic sur la barre : rendre la main à la fenêtre où l'élève écrivait
                    if GetForegroundWindow() == own_hwnd() {
                        SetForegroundWindow(hook::last_target() as HWND);
                        std::thread::sleep(Duration::from_millis(40));
                    }
                }
                inject::replace(erase, &text);
                typed.clear();
                items.clear();
                emit("", &[]);
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
        .invoke_handler(tauri::generate_handler![pick, toggle])
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
