//! Mises à jour : Scriba lit la dernière publication GitHub (Kaynegiordano/scriba), télécharge
//! le nouveau Scriba.exe à côté de lui, vérifie sa taille et son empreinte SHA-256, puis
//! remplace son propre exécutable et se relance. Rien n'est installé sans clic de l'utilisateur.
//! Tout passe par WinHTTP (voir `net`) : aucun programme caché n'est lancé.

use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

const API: &str = "https://api.github.com/repos/Kaynegiordano/scriba/releases/latest";

/// L'adresse de la dernière publication (SCRIBA_UPDATE_API la remplace : essais hors ligne).
fn api() -> String {
    std::env::var("SCRIBA_UPDATE_API").unwrap_or_else(|_| API.to_string())
}
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Argument de la nouvelle version lancée par l'ancienne : attendre que l'ancienne soit fermée.
pub const AFTER_UPDATE: &str = "--after-update";

#[derive(Clone)]
struct Pending {
    tag: String,
    url: String,
    size: u64,
    sha256: Option<String>,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

/// Ce que la barre affiche : « available » (bouton Installer), « uptodate », « error », « downloading ».
#[derive(Clone, Serialize)]
pub struct Status {
    pub state: &'static str,
    pub version: String,
    pub message: String,
    pub notes: String,
}

fn status(state: &'static str, version: &str, message: impl Into<String>, notes: &str) -> Status {
    Status { state, version: version.to_string(), message: message.into(), notes: notes.to_string() }
}

/// « v1.2.3 » -> [1, 2, 3]
fn parse_version(v: &str) -> Vec<u64> {
    v.trim().trim_start_matches(['v', 'V']).split(['.', '-']).map_while(|p| p.parse().ok()).collect()
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    let (a, b) = (parse_version(candidate), parse_version(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

/// Une version de développement (dans target\release) ne se met jamais à jour.
fn exe_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dev = exe.components().any(|c| c.as_os_str().eq_ignore_ascii_case("target"));
    (!dev).then_some(exe)
}

/// Cherche une version plus récente. `Err` : pas de réseau, dépôt injoignable…
pub fn check() -> Result<Status, String> {
    let body = crate::net::get(&api(), &["User-Agent: Scriba", "Accept: application/vnd.github+json"])?;
    let json: serde_json::Value = serde_json::from_slice(&body).map_err(|_| "Réponse illisible")?;
    let tag = json["tag_name"].as_str().ok_or("Aucune publication trouvée")?.to_string();
    if !is_newer(&tag, VERSION) {
        *PENDING.lock().unwrap() = None;
        return Ok(status("uptodate", VERSION, format!("Scriba {VERSION} est à jour."), ""));
    }
    let asset = json["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case("scriba.exe"))))
        .ok_or("La publication n'a pas de Scriba.exe")?;
    let url = asset["browser_download_url"].as_str().ok_or("Adresse de téléchargement absente")?.to_string();
    let size = asset["size"].as_u64().unwrap_or(0);
    let sha256 = asset["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).map(str::to_lowercase);
    // les notes : les premières lignes seulement
    let notes: String = json["body"].as_str().unwrap_or("").lines().filter(|l| !l.trim().is_empty()).take(6).collect::<Vec<_>>().join("\n");
    *PENDING.lock().unwrap() = Some(Pending { tag: tag.clone(), url, size, sha256 });
    Ok(status("available", tag.trim_start_matches('v'), format!("Scriba {} est disponible.", tag.trim_start_matches('v')), &notes))
}

/// Télécharge la version annoncée par `check`, remplace l'exécutable et prépare le redémarrage.
/// Renvoie `Ok(())` quand tout est en place : l'appelant doit alors quitter (Scriba se relance seul).
pub fn install(relaunch: bool) -> Result<(), String> {
    let pending = PENDING.lock().unwrap().clone().ok_or("Aucune mise à jour à installer")?;
    let exe = exe_path().ok_or("Version de développement : pas de mise à jour automatique")?;
    let dir = exe.parent().ok_or("Dossier introuvable")?.to_path_buf();
    let fresh = dir.join("Scriba.new");
    let old = dir.join("Scriba.old");
    let _ = std::fs::remove_file(&fresh);

    let (size, sha) = crate::net::download(&pending.url, &fresh).map_err(|e| format!("Le téléchargement a échoué ({e})"))?;
    if size == 0 || (pending.size > 0 && size != pending.size) {
        let _ = std::fs::remove_file(&fresh);
        return Err("Le fichier téléchargé est incomplet".into());
    }
    if let Some(expected) = &pending.sha256 {
        if sha != *expected {
            let _ = std::fs::remove_file(&fresh);
            return Err("L'empreinte du fichier ne correspond pas : mise à jour refusée".into());
        }
    }
    // un exécutable en cours d'exécution peut être renommé, pas écrasé
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old).map_err(|e| format!("Impossible de remplacer Scriba.exe ({e})"))?;
    if let Err(e) = std::fs::rename(&fresh, &exe) {
        let _ = std::fs::rename(&old, &exe);
        return Err(format!("Impossible d'installer la mise à jour ({e})"));
    }
    // KaneOS Setup compare ce fichier à la dernière publication
    let version_file = dir.join("version.txt");
    if version_file.exists() {
        let _ = std::fs::write(version_file, &pending.tag);
    }
    if !relaunch {
        return Ok(());
    }
    // la nouvelle version attend elle-même que celle-ci soit fermée (une seule instance à la fois)
    let _ = Command::new(&exe).arg(AFTER_UPDATE).spawn();
    Ok(())
}

/// Au démarrage : supprime l'ancien exécutable laissé par la mise à jour précédente.
pub fn cleanup() {
    if let Some(exe) = exe_path() {
        if let Some(dir) = exe.parent() {
            let _ = std::fs::remove_file(dir.join("Scriba.old"));
            let _ = std::fs::remove_file(dir.join("Scriba.new"));
        }
    }
}

pub fn available() -> bool {
    exe_path().is_some()
}

/// `Scriba.exe --update` : met à jour sans interface (pour les scripts), puis s'arrête.
/// Code de sortie : 0 mise à jour installée, 10 déjà à jour, 1 échec.
pub fn run_cli() -> i32 {
    match check() {
        Ok(s) if s.state == "available" => match install(false) {
            Ok(()) => 0,
            Err(_) => 1,
        },
        Ok(_) => 10,
        Err(_) => 1,
    }
}

pub fn downloading(version: &str) -> Status {
    status("downloading", version, "Téléchargement de la mise à jour…", "")
}

pub fn failed(message: impl Into<String>) -> Status {
    status("error", "", message, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("v1.0.1", "1.0.0"));
        assert!(is_newer("v1.1.0", "1.0.9"));
        assert!(is_newer("v2.0.0", "1.9.9"));
        assert!(is_newer("1.0.0", "0.1.2"));
        assert!(!is_newer("v1.0.0", "1.0.0"));
        assert!(!is_newer("v0.1.2", "1.0.0"));
        assert!(!is_newer("v1.0", "1.0.0"));
    }
}
