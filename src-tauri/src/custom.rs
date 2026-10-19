//! Réglages et dictionnaire personnel, enregistrés dans %APPDATA%\Scriba :
//!   settings.txt      auto=0|1|2   (0 = aucune, 1 = orthographe, 2 = orthographe + grammaire)
//!                     hidden=0|1   démarrer avec la barre masquée (1 par défaut)
//!                     doublespace=0|1  deux espaces = un point (1 par défaut)
//!                     audio=0|1    lecture à voix haute (désactivée par défaut)
//!                     pictos=0|1|2 pictogrammes (0 = non, 1 = ceux du PC, 2 = + Internet pour les mots rares)
//!   dictionnaire.txt  un mot par ligne (mot correct) ou « faux>juste » (correction refusée)

use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

pub const OFF: u8 = 0;
pub const SPELLING: u8 = 1;
pub const FULL: u8 = 2;

static MODE: AtomicU8 = AtomicU8::new(FULL);
static AUDIO: AtomicBool = AtomicBool::new(false);
static PICTOS: AtomicU8 = AtomicU8::new(2);
static HIDDEN: AtomicBool = AtomicBool::new(true);
static DOUBLE_SPACE: AtomicBool = AtomicBool::new(true);

#[derive(Default)]
struct Custom {
    words: HashSet<String>,
    ignore: HashSet<String>,
}

static CUSTOM: OnceLock<Mutex<Custom>> = OnceLock::new();

fn custom() -> &'static Mutex<Custom> {
    CUSTOM.get_or_init(|| Mutex::new(Custom::default()))
}

fn dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Scriba"))
}

pub fn load() {
    let Some(d) = dir() else { return };
    if let Ok(t) = std::fs::read_to_string(d.join("settings.txt")) {
        for line in t.lines() {
            if let Some(v) = line.strip_prefix("pictos=") {
                if let Ok(m) = v.trim().parse::<u8>() {
                    PICTOS.store(m.min(2), Ordering::Relaxed);
                }
            }
            if let Some(v) = line.strip_prefix("doublespace=") {
                DOUBLE_SPACE.store(v.trim() != "0", Ordering::Relaxed);
            }
            if let Some(v) = line.strip_prefix("hidden=") {
                HIDDEN.store(v.trim() != "0", Ordering::Relaxed);
            }
            if let Some(v) = line.strip_prefix("audio=") {
                AUDIO.store(v.trim() == "1", Ordering::Relaxed);
            }
            if let Some(v) = line.strip_prefix("auto=") {
                if let Ok(m) = v.trim().parse::<u8>() {
                    MODE.store(m.min(FULL), Ordering::Relaxed);
                }
            }
        }
    }
    if let Ok(t) = std::fs::read_to_string(d.join("dictionnaire.txt")) {
        let mut c = custom().lock().unwrap();
        for line in t.lines() {
            let l = line.trim().to_lowercase();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            if l.contains('>') {
                c.ignore.insert(l);
            } else {
                c.words.insert(l);
            }
        }
    }
}

pub fn mode() -> u8 {
    MODE.load(Ordering::Relaxed)
}

pub fn set_mode(m: u8) {
    MODE.store(m.min(FULL), Ordering::Relaxed);
    save();
}

pub fn audio() -> bool {
    AUDIO.load(Ordering::Relaxed)
}

pub fn set_audio(on: bool) {
    AUDIO.store(on, Ordering::Relaxed);
    save();
}

pub fn double_space() -> bool {
    DOUBLE_SPACE.load(Ordering::Relaxed)
}

pub fn set_double_space(on: bool) {
    DOUBLE_SPACE.store(on, Ordering::Relaxed);
    save();
}

pub fn start_hidden() -> bool {
    HIDDEN.load(Ordering::Relaxed)
}

pub fn set_start_hidden(on: bool) {
    HIDDEN.store(on, Ordering::Relaxed);
    save();
}

pub fn pictos() -> u8 {
    PICTOS.load(Ordering::Relaxed)
}

pub fn set_pictos(m: u8) {
    PICTOS.store(m.min(2), Ordering::Relaxed);
    save();
}

fn save() {
    if cfg!(test) {
        return;
    }
    if let Some(d) = dir() {
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join("settings.txt"), format!("auto={}\naudio={}\npictos={}\nhidden={}\ndoublespace={}\n", mode(), u8::from(audio()), pictos(), u8::from(start_hidden()), u8::from(double_space())));
    }
}

/// Mot ajouté par l'élève : jamais signalé comme faute.
pub fn knows(word: &str) -> bool {
    custom().lock().unwrap().words.contains(&word.to_lowercase())
}

/// Correction « faux -> juste » refusée par l'élève.
pub fn ignored(original: &str, replacement: &str) -> bool {
    custom().lock().unwrap().ignore.contains(&format!("{}>{}", original.to_lowercase(), replacement.to_lowercase()))
}

pub fn add_word(word: &str) {
    let w = word.to_lowercase();
    if custom().lock().unwrap().words.insert(w.clone()) {
        append(&w);
    }
}

pub fn add_ignore(original: &str, replacement: &str) {
    let l = format!("{}>{}", original.to_lowercase(), replacement.to_lowercase());
    if custom().lock().unwrap().ignore.insert(l.clone()) {
        append(&l);
    }
}

fn append(line: &str) {
    if cfg!(test) {
        return;
    }
    let Some(d) = dir() else { return };
    let _ = std::fs::create_dir_all(&d);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(d.join("dictionnaire.txt")) {
        let _ = writeln!(f, "{line}");
    }
}
