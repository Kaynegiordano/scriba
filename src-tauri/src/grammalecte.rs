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

fn sha256_of(path: &PathBuf) -> Option<String> {
    let out = Command::new("certutil.exe").args(["-hashfile"]).arg(path).arg("SHA256").creation_flags(CREATE_NO_WINDOW).output().ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.replace(' ', "").to_lowercase())
        .find(|l| l.len() == 64 && l.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Premier lancement : télécharge et décompresse Grammalecte (≈ 6 Mo), vérifié par son empreinte.
fn install(dir: &PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let zip = dir.with_extension("zip");
    let ok = Command::new("curl.exe")
        .args(["-sSL", "-m", "300", "-H", "User-Agent: Scriba", "-o"])
        .arg(&zip)
        .arg(URL)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return Err("téléchargement impossible".into());
    }
    if sha256_of(&zip).as_deref() != Some(SHA256) {
        let _ = std::fs::remove_file(&zip);
        return Err("empreinte inattendue".into());
    }
    let ok = Command::new("tar.exe").arg("-xf").arg(&zip).arg("-C").arg(dir).creation_flags(CREATE_NO_WINDOW).status().is_ok_and(|s| s.success());
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
        let ready = run();
        if !ready {
            // on réessaiera au prochain lancement de Scriba ; d'ici là, les règles seules
        }
    });
}

fn run() -> bool {
    let Some(dir) = package_dir() else { return false };
    if ready_dir().is_none() && std::env::var_os("SCRIBA_GRAMMALECTE").is_none() && install(&dir).is_err() {
        return false;
    }
    let Some((py, pre)) = python() else { return false };
    let script = dir.join("scriba_gc.py");
    if std::fs::read_to_string(&script).ok().as_deref() != Some(SCRIPT) && std::fs::write(&script, SCRIPT).is_err() {
        return false;
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
    let Ok(mut child) = child else { return false };
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else { return false };
    let mut lines = BufReader::new(stdout);
    let mut first = String::new();
    if lines.read_line(&mut first).is_err() || first.trim() != "ready" {
        let _ = child.kill();
        return false;
    }
    let (tx, rx) = channel::<Request>();
    *state().lock().unwrap() = State::Ready(tx);
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
    *state().lock().unwrap() = State::Off;
    false
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
