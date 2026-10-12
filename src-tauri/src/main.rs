#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(test)]
mod auto_tests;
mod clipboard;
mod conj;
mod custom;
mod grammar;
mod hook;
mod inject;
mod lexicon;
mod phonetics;

use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use std::sync::{Mutex, OnceLock};

use conj::{Conjugator, Table};
use grammar::Correction;
use hook::Msg;
use lexicon::{Hint, Lexicon, Suggestion};
use serde::Serialize;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

static LEX: OnceLock<Lexicon> = OnceLock::new();
static CONJ: OnceLock<Conjugator> = OnceLock::new();
/// Le texte affiché dans la fenêtre de lecture.
static READING: Mutex<String> = Mutex::new(String::new());
static AUDIO_ITEM: OnceLock<CheckMenuItem<tauri::Wry>> = OnceLock::new();

/// Le dictionnaire s'il est déjà chargé (le clavier ne doit jamais attendre son chargement).
pub fn try_lex() -> Option<&'static Lexicon> {
    LEX.get()
}

fn lex() -> &'static Lexicon {
    LEX.get_or_init(|| Lexicon::load_with(include_str!("../data/lexique.tsv"), include_str!("../data/extra.txt")))
}

fn conjugator() -> &'static Conjugator {
    CONJ.get_or_init(|| Conjugator::load(include_str!("../data/conj.tsv")))
}
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
    /// mot inconnu : F8 peut l'ajouter au dictionnaire
    learnable: bool,
}

/// Une correction automatique vient d'être faite (ou annulée).
#[derive(Clone, Serialize)]
struct Notice {
    from: String,
    to: String,
    reason: String,
    undone: bool,
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

#[derive(Clone, Serialize)]
struct Settings {
    auto: u8,
    audio: bool,
}

#[tauri::command]
fn get_settings() -> Settings {
    Settings { auto: custom::mode(), audio: custom::audio() }
}

#[tauri::command]
fn set_audio(app: AppHandle, on: bool) {
    apply_audio(&app, on);
}

fn apply_audio(app: &AppHandle, on: bool) {
    custom::set_audio(on);
    if let Some(item) = AUDIO_ITEM.get() {
        let _ = item.set_checked(on);
    }
    let _ = app.emit("settings", Settings { auto: custom::mode(), audio: on });
}

#[tauri::command]
fn open_reader(tx: State<Tx>) {
    let _ = tx.0.send(Msg::Read);
}

#[tauri::command]
fn clipboard_text() -> String {
    clipboard::get_text().unwrap_or_default()
}

#[tauri::command]
fn reading_text() -> String {
    READING.lock().unwrap().clone()
}

/// Un morceau de texte pour la fenêtre de lecture : un mot (avec ses syllabes) ou ce qui les sépare.
#[derive(Serialize)]
struct Piece {
    text: String,
    syll: Vec<String>,
}

#[tauri::command]
fn syllabify(text: String) -> Vec<Piece> {
    let lex = lex();
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Piece> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_alphabetic() {
            let start = i;
            while i < chars.len() && !chars[i].is_alphabetic() {
                i += 1;
            }
            out.push(Piece { text: chars[start..i].iter().collect(), syll: Vec::new() });
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_alphabetic() {
            i += 1;
        }
        // « aujourd'hui » reste entier ; « l'école » donne « l' » + « école »
        if i + 1 < chars.len() && matches!(chars[i], '\'' | '’') && chars[i + 1].is_alphabetic() {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_alphabetic() {
                j += 1;
            }
            let glued: String = chars[start..j].iter().map(|c| if *c == '’' { '\'' } else { *c }).collect();
            if lex.lookup(&glued).is_some() {
                i = j;
            } else {
                i += 1; // l'apostrophe reste avec « l' »
            }
        }
        let word: String = chars[start..i].iter().collect();
        let syll = if word.ends_with(['\'', '’']) { vec![word.clone()] } else { lex.syllables(&word) };
        out.push(Piece { text: word, syll });
    }
    out
}

/// Le texte sélectionné dans l'application (Ctrl+C simulé, presse-papiers rendu tel qu'il était),
/// sinon la phrase en cours de frappe.
fn grab_text() -> String {
    if hook::target_is_writing_app() {
        let before = clipboard::sequence();
        let saved = clipboard::get_text();
        inject::copy_selection();
        let start = std::time::Instant::now();
        while clipboard::sequence() == before && start.elapsed() < Duration::from_millis(400) {
            std::thread::sleep(Duration::from_millis(15));
        }
        if clipboard::sequence() != before {
            let text = clipboard::get_text().unwrap_or_default();
            if let Some(prev) = saved {
                clipboard::set_text(&prev);
            }
            if !text.trim().is_empty() {
                return text;
            }
        }
    }
    hook::current_sentence()
}

fn show_reader(app: &AppHandle, text: String) {
    *READING.lock().unwrap() = text.clone();
    if let Some(w) = app.get_webview_window("lire") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.emit_to("lire", "reader-text", text);
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "lire", WebviewUrl::App("index.html#lire".into()))
        .title("Scriba — Lecture")
        .inner_size(960.0, 720.0)
        .min_inner_size(560.0, 400.0)
        .decorations(false)
        .center()
        .focused(true)
        .build();
}

#[tauri::command]
fn learn(tx: State<Tx>) {
    let _ = tx.0.send(Msg::Learn);
}

#[tauri::command]
fn toggle(tx: State<Tx>) {
    let _ = tx.0.send(Msg::Toggle);
}

#[tauri::command]
fn open_conj(tx: State<Tx>) {
    let _ = tx.0.send(Msg::Conj);
}

#[tauri::command]
fn conj_table(verb: String) -> Option<Table> {
    conjugator().conjugate(verb.trim())
}

#[derive(Serialize)]
struct Choice {
    verb: String,
    /// « forme de manger », vide si l'élève a tapé l'infinitif
    hint: String,
}

/// Ce que l'élève cherche : un infinitif, une forme conjuguée (« mangeaient »),
/// ou un mot mal écrit (« manjé »).
#[tauri::command]
fn conj_search(query: String) -> Vec<Choice> {
    let q = query.trim().to_lowercase();
    let (lex, conj) = (lex(), conjugator());
    let mut out: Vec<Choice> = Vec::new();
    let mut push = |verb: &str, hint: String| {
        if conj.knows(verb) && !out.iter().any(|c| c.verb == verb) && out.len() < 8 {
            out.push(Choice { verb: verb.to_string(), hint });
        }
    };
    if q.is_empty() {
        return out;
    }
    push(&q, String::new());
    if let Some(e) = lex.lookup(&q) {
        for (l, _) in &e.verbs {
            push(lex.lemma_name(*l), format!("« {q} » vient de ce verbe"));
        }
    }
    for v in conj.search(&q, 8) {
        push(&v, String::new());
    }
    let hint = Hint { infinitive: true, ..Default::default() };
    for s in lex.suggest(&q, &hint, 12) {
        push(&s.word.to_lowercase(), String::new());
        if let Some(e) = lex.lookup(&s.word) {
            for (l, _) in &e.verbs {
                push(lex.lemma_name(*l), String::new());
            }
        }
    }
    out
}

/// Le dernier verbe de la phrase en cours (pour ouvrir la conjugaison dessus).
fn last_verb(sentence: &[char]) -> String {
    let (lex, conj) = (lex(), conjugator());
    for t in grammar::tokenize(lex, sentence).iter().rev() {
        if let Some(e) = lex.lookup(&t.lower) {
            if matches!(e.top_tag(), "VER" | "AUX") {
                if let Some(l) = e.verbs.iter().map(|(l, _)| lex.lemma_name(*l)).find(|l| conj.knows(l)) {
                    return l.to_string();
                }
            }
        }
    }
    String::new()
}

fn show_conj(app: &AppHandle, verb: &str) {
    if let Some(w) = app.get_webview_window("conj") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        if !verb.is_empty() {
            let _ = w.emit_to("conj", "conj-open", verb.to_string());
        }
        return;
    }
    let url = format!("index.html#conj={}", verb.replace(' ', "%20"));
    let _ = WebviewWindowBuilder::new(app, "conj", WebviewUrl::App(url.into()))
        .title("Scriba — Conjugaison")
        .inner_size(900.0, 700.0)
        .min_inner_size(560.0, 420.0)
        .decorations(false)
        .center()
        .focused(true)
        .build();
}

/// « a, mange » -> « à, mangent » pour la pastille.
fn pairs_view(pairs: &[(String, String, &'static str)]) -> (String, String) {
    let from: Vec<&str> = pairs.iter().map(|p| p.0.as_str()).collect();
    let to: Vec<&str> = pairs.iter().map(|p| p.1.as_str()).collect();
    (from.join(", "), to.join(", "))
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
    let lex = lex();
    conjugator();
    let mut sentence: Vec<char> = Vec::new();
    let mut typed = String::new();
    let mut items: Vec<Suggestion> = Vec::new();
    let mut fix: Option<Correction> = None;

    let refresh = |sentence: &[char], typed: &mut String, items: &mut Vec<Suggestion>, fix: &mut Option<Correction>| {
        let word_len = sentence.iter().rev().take_while(|c| c.is_alphabetic()).count();
        *typed = sentence[sentence.len() - word_len..].iter().collect();

        let toks = grammar::tokenize(lex, sentence);
        // le mot en cours est le dernier jeton s'il touche la fin de la phrase
        let cur = toks.len().saturating_sub(if word_len > 0 { 1 } else { 0 });
        *fix = grammar::check(lex, sentence)
            .into_iter()
            .rev()
            .find(|c| sentence.len() - c.start <= MAX_FIX_DISTANCE);
        let max = if fix.is_some() { MAX_SUGGESTIONS - 1 } else { MAX_SUGGESTIONS };
        *items = if typed.is_empty() {
            Vec::new()
        } else {
            let h = grammar::hint(lex, &toks, cur.min(toks.len().saturating_sub(1)));
            lex.suggest(typed, &h, max)
        };
        for s in items.iter_mut() {
            s.word = match_case(typed, &s.word);
        }
        hook::CORRECTION.store(fix.is_some(), Ordering::Relaxed);
        let learnable = fix.as_ref().is_some_and(|c| c.reason.starts_with("Mot inconnu"));
        hook::LEARNABLE.store(learnable, Ordering::Relaxed);
        let view = fix.as_ref().map(|c| Fix {
            from: c.original.clone(),
            to: c.replacement.clone(),
            reason: c.reason.clone(),
            kind: c.kind,
            learnable,
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
            Msg::Auto(a) => {
                // on retape depuis le premier mot corrigé ; l'espace (ou la ponctuation) avalée est retapée aussi
                hook::set_tail(a.erase, &a.text, false);
                inject::replace(a.erase, &a.text);
                let keep = sentence.len().saturating_sub(a.erase);
                sentence.truncate(keep);
                sentence.extend(a.text.chars());
                let (from, to) = pairs_view(&a.pairs);
                let _ = app.emit("auto", Notice { from, to, reason: a.reason.clone(), undone: false });
                hook::set_last_auto(Some(a));
                refresh(&sentence, &mut typed, &mut items, &mut fix);
                hook::finish_hold();
            }
            Msg::Undo(a) => {
                // Retour arrière juste après une correction : on remet ce que l'élève avait tapé, et on s'en souvient
                let erase = a.text.chars().count();
                hook::set_tail(erase, &a.undo, false);
                inject::replace(erase, &a.undo);
                let keep = sentence.len().saturating_sub(erase);
                sentence.truncate(keep);
                sentence.extend(a.undo.chars());
                for (orig, repl, kind) in &a.pairs {
                    if *kind == "orthographe" {
                        custom::add_word(orig);
                    } else if orig.chars().count() > 3 {
                        // a / et / sa / ce… : refuser une fois ne veut pas dire « jamais »
                        custom::add_ignore(orig, repl);
                    }
                }
                let (to, from) = pairs_view(&a.pairs);
                let _ = app.emit(
                    "auto",
                    Notice { from, to, reason: "D'accord, je ne le corrigerai plus.".into(), undone: true },
                );
                refresh(&sentence, &mut typed, &mut items, &mut fix);
                hook::finish_hold();
            }
            Msg::Learn => {
                if let Some(c) = fix.take() {
                    custom::add_word(&c.original);
                    refresh(&sentence, &mut typed, &mut items, &mut fix);
                }
            }
            Msg::Toggle => set_visible(!hook::VISIBLE.load(Ordering::Relaxed)),
            Msg::Conj => show_conj(&app, &last_verb(&sentence)),
            Msg::Read => {
                let text = grab_text();
                show_reader(&app, text);
            }
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

    custom::load();
    let (tx, rx) = channel::<Msg>();
    let hook_tx = tx.clone();
    let tray_tx = tx.clone();

    tauri::Builder::default()
        .manage(Tx(tx))
        .invoke_handler(tauri::generate_handler![
            pick, apply_fix, learn, toggle, open_conj, conj_table, conj_search, get_settings, set_audio, open_reader,
            reading_text, syllabify, clipboard_text
        ])
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
            // « Réduire tout » (Win+D, Win+M) ne doit pas faire disparaître la barre
            let bar = win.clone();
            win.on_window_event(move |e| {
                if let tauri::WindowEvent::Resized(_) = e {
                    if bar.is_minimized().unwrap_or(false) && hook::VISIBLE.load(Ordering::Relaxed) {
                        let _ = bar.unminimize();
                        set_visible(true);
                    }
                }
            });

            let toggle_item = MenuItem::with_id(app, "toggle", "Afficher / masquer   Ctrl+Alt+L", true, None::<&str>)?;
            let conj_item = MenuItem::with_id(app, "conj", "Conjugaison   Ctrl+Alt+C", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quitter Scriba", true, None::<&str>)?;
            let mode = custom::mode();
            let auto_items = [
                CheckMenuItem::with_id(app, "auto0", "Désactivée", true, mode == custom::OFF, None::<&str>)?,
                CheckMenuItem::with_id(app, "auto1", "Orthographe seulement", true, mode == custom::SPELLING, None::<&str>)?,
                CheckMenuItem::with_id(app, "auto2", "Orthographe + grammaire", true, mode == custom::FULL, None::<&str>)?,
            ];
            let auto_menu = Submenu::with_items(
                app,
                "Correction automatique",
                true,
                &[&auto_items[0], &auto_items[1], &auto_items[2]],
            )?;
            let read_item = MenuItem::with_id(app, "read", "Lecture   Ctrl+Alt+R", true, None::<&str>)?;
            let audio_item = CheckMenuItem::with_id(app, "audio", "Lecture audio des mots", true, custom::audio(), None::<&str>)?;
            let _ = AUDIO_ITEM.set(audio_item.clone());
            let menu = Menu::with_items(
                app,
                &[&toggle_item, &conj_item, &read_item, &auto_menu, &audio_item, &quit_item],
            )?;
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
                    "conj" => {
                        let _ = menu_tx.send(Msg::Conj);
                    }
                    "read" => {
                        let _ = menu_tx.send(Msg::Read);
                    }
                    "audio" => apply_audio(app, !custom::audio()),
                    id @ ("auto0" | "auto1" | "auto2") => {
                        let m = id.as_bytes()[4] - b'0';
                        custom::set_mode(m);
                        for (i, item) in auto_items.iter().enumerate() {
                            let _ = item.set_checked(i as u8 == m);
                        }
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
