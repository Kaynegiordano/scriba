//! Grammalecte (grammalecte.net, licence GPL 3) : le correcteur grammatical du français de LibreOffice et Firefox.
//! Il lit toute la phrase (accords dans le groupe du nom, conjugaison avec le sujet, participes, confusions comme
//! a/à ou ces/ses) là où les règles de Scriba ne voient que quelques mots. Scriba garde sa correction des mots
//! (phonétique, fautes de frappe) ; Grammalecte s'occupe de la grammaire.
//!
//! Il tourne dans un processus Python à part (scriba_gc.py, embarqué ci-dessous), interrogé par une ligne JSON.
//! Rien n'est bloquant : tant que Grammalecte n'est pas prêt (téléchargement au premier lancement, chargement
//! d'environ une seconde), ou si Python est absent, Scriba corrige avec ses seules règles.
//! Installé dans %LOCALAPPDATA%\Scriba\grammalecte-<version>, téléchargé depuis grammalecte.net et vérifié par SHA-256.
//! SCRIBA_GRAMMALECTE=<dossier du paquet> (tests) ; SCRIBA_GRAMMALECTE=off le désactive.

use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const VERSION: &str = "2.3.0";
const URL: &str = "https://grammalecte.net/zip/Grammalecte-fr-v2.3.0.zip";
const SHA256: &str = "aaa4219704857778038ecc1db18f6a907994c0125b8e762dcecc69130280684f";
/// Au-delà, la frappe n'attend pas : la phrase est corrigée par les seules règles de Scriba.
const TIMEOUT: Duration = Duration::from_millis(150);
const SCRIPT: &str = include_str!("scriba_gc.py");

#[derive(Clone, Debug)]
pub struct Error {
    /// en caractères dans le texte envoyé
    pub start: usize,
    pub end: usize,
    /// option de Grammalecte : gn, conj, ppas, conf, infi…
    pub kind: String,
    pub sugg: Vec<String>,
    pub msg: String,
}

type Request = (String, Sender<Vec<Error>>);

enum State {
    Off,
    Starting,
    Ready(Sender<Request>),
}

/// Pourquoi Grammalecte ne tourne pas (pour le menu de Scriba).
static PROBLEM: Mutex<&'static str> = Mutex::new("");
/// Prévenu à chaque changement d'état (le menu affiche alors `status()`).
static ON_CHANGE: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

pub fn on_change(f: impl Fn() + Send + Sync + 'static) {
    let _ = ON_CHANGE.set(Box::new(f));
}

fn set_state(s: State, problem: &'static str) {
    *state().lock().unwrap() = s;
    *PROBLEM.lock().unwrap() = problem;
    if let Some(f) = ON_CHANGE.get() {
        f();
    }
}

/// Une ligne pour le menu : où en est la grammaire de la phrase entière.
pub fn status() -> String {
    match &*state().lock().unwrap() {
        State::Ready(_) => "Grammaire avancée : active".into(),
        State::Starting => "Grammaire avancée : démarrage…".into(),
        State::Off => {
            let why = *PROBLEM.lock().unwrap();
            format!("Grammaire avancée : arrêtée{}", if why.is_empty() { String::new() } else { format!(" ({why})") })
        }
    }
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(State::Off))
}

fn cache() -> &'static Mutex<(String, Vec<Error>)> {
    static C: OnceLock<Mutex<(String, Vec<Error>)>> = OnceLock::new();
    C.get_or_init(|| Mutex::new((String::new(), Vec::new())))
}

fn package_dir() -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("SCRIBA_GRAMMALECTE") {
        return (v != "off").then(|| PathBuf::from(v));
    }
    std::env::var_os("LOCALAPPDATA").map(|a| PathBuf::from(a).join("Scriba").join(format!("grammalecte-{VERSION}")))
}

fn ready_dir() -> Option<PathBuf> {
    let d = package_dir()?;
    d.join("grammalecte").join("__init__.py").is_file().then_some(d)
}

/// Le vrai python.exe (pas l'alias du Microsoft Store), sinon le lanceur py.exe.
fn python() -> Option<(PathBuf, Vec<&'static str>)> {
    if let Some(p) = std::env::var_os("SCRIBA_PYTHON") {
        return Some((PathBuf::from(p), vec![]));
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let exe = dir.join("python.exe");
        if exe.is_file() && !dir.to_string_lossy().to_lowercase().contains("windowsapps") {
            return Some((exe, vec![]));
        }
    }
    let windir = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let py = windir.join("py.exe");
    py.is_file().then_some((py, vec!["-3"]))
}

/// Décompresse l'archive dans `dir` (les chemins qui sortiraient du dossier sont refusés).
fn unzip(zip: &PathBuf, dir: &PathBuf) -> Result<(), String> {
    let file = std::fs::File::open(zip).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    archive.extract(dir).map_err(|e| e.to_string())
}

/// Premier lancement : télécharge et décompresse Grammalecte (≈ 6 Mo), vérifié par son empreinte.
fn install(dir: &PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let zip = dir.with_extension("zip");
    let (_, sha) = crate::net::download(URL, &zip)?;
    if sha != SHA256 {
        let _ = std::fs::remove_file(&zip);
        return Err("empreinte inattendue".into());
    }
    let ok = unzip(&zip, dir).is_ok();
    let _ = std::fs::remove_file(&zip);
    if ok && dir.join("grammalecte").join("__init__.py").is_file() {
        Ok(())
    } else {
        let _ = std::fs::remove_dir_all(dir);
        Err("décompression impossible".into())
    }
}

/// Démarre Grammalecte en arrière-plan (téléchargement compris). Sans effet s'il tourne déjà.
pub fn start() {
    {
        let mut s = state().lock().unwrap();
        if !matches!(*s, State::Off) {
            return;
        }
        *s = State::Starting;
    }
    std::thread::spawn(|| {
        // le processus Python peut s'arrêter (mise en veille, antivirus, mémoire) : on le relance, quelques fois
        // seulement ; s'il n'a jamais démarré (Python absent, pas de réseau), on réessaiera au prochain lancement
        for attempt in 0..5 {
            let (was_ready, problem) = run();
            set_state(State::Off, problem);
            if !was_ready {
                return;
            }
            std::thread::sleep(Duration::from_secs(5 * (attempt + 1)));
            set_state(State::Starting, "");
        }
    });
}

/// Fait tourner Grammalecte jusqu'à son arrêt. Renvoie (il a été prêt, pourquoi il ne tourne plus).
fn run() -> (bool, &'static str) {
    let Some(dir) = package_dir() else { return (false, "désactivée") };
    // Python d'abord : inutile de télécharger Grammalecte sur un PC qui ne pourra pas le lancer
    let Some((py, pre)) = python() else { return (false, "Python n'est pas installé") };
    if ready_dir().is_none() && std::env::var_os("SCRIBA_GRAMMALECTE").is_none() && install(&dir).is_err() {
        return (false, "téléchargement impossible");
    }
    let script = dir.join("scriba_gc.py");
    if std::fs::read_to_string(&script).ok().as_deref() != Some(SCRIPT) && std::fs::write(&script, SCRIPT).is_err() {
        return (false, "dossier non accessible");
    }
    let child = Command::new(py)
        .args(pre)
        .arg("-I")
        .arg("-u")
        .arg(&script)
        .arg(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    let Ok(mut child) = child else { return (false, "Python ne démarre pas") };
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else { return (false, "Python ne démarre pas") };
    let mut lines = BufReader::new(stdout);
    let mut first = String::new();
    if lines.read_line(&mut first).is_err() || first.trim() != "ready" {
        let _ = child.kill();
        let _ = child.wait();
        return (false, "Python ne démarre pas");
    }
    let (tx, rx) = channel::<Request>();
    set_state(State::Ready(tx), "");
    // une requête à la fois : chaque réponse correspond toujours à sa question
    for (text, reply) in rx {
        let Ok(q) = serde_json::to_string(&text) else { continue };
        if writeln!(stdin, "{q}").is_err() || stdin.flush().is_err() {
            break;
        }
        let mut line = String::new();
        if lines.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let _ = reply.send(parse(&line));
    }
    let _ = child.kill();
    let _ = child.wait();
    (true, "arrêt inattendu")
}

fn parse(line: &str) -> Vec<Error> {
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(line) else { return Vec::new() };
    items
        .iter()
        .filter_map(|e| {
            let a = e.as_array()?;
            Some(Error {
                start: a.first()?.as_u64()? as usize,
                end: a.get(1)?.as_u64()? as usize,
                kind: a.get(2)?.as_str()?.to_string(),
                sugg: a.get(3)?.as_array()?.iter().filter_map(|s| s.as_str().map(str::to_string)).collect(),
                msg: a.get(4)?.as_str().unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// Les erreurs de grammaire du texte, ou None si Grammalecte n'est pas (encore) disponible ou trop lent.
pub fn errors(text: &str) -> Option<Vec<Error>> {
    {
        let c = cache().lock().unwrap();
        if c.0 == text {
            return Some(c.1.clone());
        }
    }
    let tx = match &*state().lock().unwrap() {
        State::Ready(tx) => tx.clone(),
        _ => return None,
    };
    let (rtx, rrx) = channel();
    tx.send((text.to_string(), rtx)).ok()?;
    let errs = rrx.recv_timeout(TIMEOUT).ok()?;
    *cache().lock().unwrap() = (text.to_string(), errs.clone());
    Some(errs)
}

/// Pour les tests : attend que Grammalecte soit prêt (au plus `secs` secondes).
#[cfg(test)]
pub fn wait_ready(secs: u64) -> bool {
    start();
    for _ in 0..secs * 20 {
        if matches!(*state().lock().unwrap(), State::Ready(_)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}
