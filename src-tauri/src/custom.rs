//! Réglages et dictionnaire personnel, enregistrés dans %APPDATA%\Scriba :
//!   settings.txt      auto=0|1|2   (0 = aucune, 1 = orthographe, 2 = orthographe + grammaire)
//!                     hidden=0|1   démarrer avec la barre masquée (1 par défaut)
//!                     doublespace=0|1  deux espaces = un point (1 par défaut)
//!                     audio=0|1    lecture à voix haute (désactivée par défaut)
//!                     pictos=0|1|2 pictogrammes (0 = non, 1 = ceux du PC, 2 = + Internet pour les mots rares)
//!   dictionnaire.txt  un mot par ligne (mot correct) ou « faux>juste » (correction refusée)

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

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
    /// refusé d'un retour arrière : laissé tranquille un moment seulement (mot ou « faux>juste »)
    soon: HashMap<String, Instant>,
    /// combien de fois chaque mot a été refusé depuis le démarrage
    refused: HashMap<String, u8>,
}

/// Une correction annulée d'un retour arrière n'est pas refaite pendant ce temps.
const FORGET: Duration = Duration::from_secs(600);
/// En tête du dictionnaire nettoyé (les versions d'avant 1.0.10 apprenaient chaque retour arrière).
const HEADER: &str = "# Scriba 2 : mots ajoutés (F8, ou refusés deux fois)";

impl Custom {
    fn fresh(&self, key: &str) -> bool {
        self.soon.get(key).is_some_and(|t| t.elapsed() < FORGET)
    }
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
    let w = word.to_lowercase();
    let c = custom().lock().unwrap();
    c.words.contains(&w) || c.fresh(&w)
}

/// Correction « faux -> juste » refusée par l'élève.
pub fn ignored(original: &str, replacement: &str) -> bool {
    let key = format!("{}>{}", original.to_lowercase(), replacement.to_lowercase());
    let c = custom().lock().unwrap();
    c.ignore.contains(&key) || c.fresh(&key)
}

/// Retour arrière juste après la correction d'un mot : on ne la refait pas tout de suite. Le mot n'est appris
/// (pour toujours) que refusé une deuxième fois, et seulement s'il ne ressemble pas à un mot courant mal tapé
/// (`learnable`) : un retour arrière est souvent un réflexe (effacer l'espace, reprendre la phrase), pas un « non ».
pub fn refuse_word(word: &str, learnable: bool) {
    let w = word.to_lowercase();
    let mut c = custom().lock().unwrap();
    c.soon.insert(w.clone(), Instant::now());
    let n = c.refused.entry(w.clone()).or_insert(0);
    *n += 1;
    if *n >= 2 && learnable && c.words.insert(w.clone()) {
        drop(c);
        append(&w);
    }
}

/// Retour arrière juste après une correction de grammaire : jamais retenu pour toujours (« je prend » est
/// une faute dans presque toutes les phrases), seulement pour un moment.
pub fn refuse_pair(original: &str, replacement: &str) {
    let key = format!("{}>{}", original.to_lowercase(), replacement.to_lowercase());
    custom().lock().unwrap().soon.insert(key, Instant::now());
}

/// Nettoie, une fois, un dictionnaire écrit par une version d'avant 1.0.10 : chaque retour arrière y
/// ajoutait la faute (« avan », « fair ») ou bloquait une règle (« prend>prends »). Ne garde que les mots
/// que `keep` accepte ; l'ancien fichier est conservé à côté.
pub fn tidy(keep: impl Fn(&str) -> bool) {
    if cfg!(test) {
        return;
    }
    let Some(d) = dir() else { return };
    let path = d.join("dictionnaire.txt");
    let Ok(t) = std::fs::read_to_string(&path) else { return };
    if t.lines().next().is_some_and(|l| l.trim() == HEADER) {
        return;
    }
    let _ = std::fs::write(d.join("dictionnaire.avant-1.0.10.txt"), &t);
    let mut c = custom().lock().unwrap();
    c.ignore.clear();
    c.words.retain(|w| keep(w));
    let mut words: Vec<&String> = c.words.iter().collect();
    words.sort();
    let mut out = format!("{HEADER}\n");
    for w in words {
        out.push_str(w);
        out.push('\n');
    }
    let _ = std::fs::write(&path, out);
}

pub fn add_word(word: &str) {
    let w = word.to_lowercase();
    if custom().lock().unwrap().words.insert(w.clone()) {
        append(&w);
    }
}

fn append(line: &str) {
    if cfg!(test) {
        return;
    }
    let Some(d) = dir() else { return };
    let _ = std::fs::create_dir_all(&d);
    let path = d.join("dictionnaire.txt");
    let new = !path.exists();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        if new {
            let _ = writeln!(f, "{HEADER}");
        }
        let _ = writeln!(f, "{line}");
    }
}
