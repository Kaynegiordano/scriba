//! Téléchargements en HTTPS avec WinHTTP (livré avec Windows) et empreinte SHA-256, sans lancer aucun
//! programme caché : `curl.exe` et `certutil.exe` lancés sans fenêtre ressemblent, pour un antivirus, à ce
//! que fait un logiciel malveillant.

use sha2::{Digest, Sha256};
use std::ffi::c_void;
use std::io::Write;
use std::path::Path;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Networking::WinHttp::*;

struct Handle(*mut c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// GET `url` (https seulement, redirections suivies) ; chaque morceau reçu est donné à `sink`.
/// `idle_secs` : délai sans rien recevoir avant d'abandonner.
fn request(url: &str, headers: &[&str], idle_secs: i32, mut sink: impl FnMut(&[u8]) -> Result<(), String>) -> Result<(), String> {
    let rest = url.strip_prefix("https://").ok_or("Adresse non sécurisée")?;
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let offline = || "Pas de connexion à Internet".to_string();
    unsafe {
        let session = Handle(WinHttpOpen(wide("Scriba").as_ptr(), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, null(), null(), 0));
        if session.0.is_null() {
            return Err(offline());
        }
        WinHttpSetTimeouts(session.0, 15_000, 15_000, 15_000, idle_secs * 1000);
        let connect = Handle(WinHttpConnect(session.0, wide(host).as_ptr(), 443, 0));
        if connect.0.is_null() {
            return Err(offline());
        }
        let req = Handle(WinHttpOpenRequest(
            connect.0,
            wide("GET").as_ptr(),
            wide(path).as_ptr(),
            null(),
            null(),
            null(),
            WINHTTP_FLAG_SECURE,
        ));
        if req.0.is_null() {
            return Err(offline());
        }
        let head = wide(&headers.join("\r\n"));
        let (hp, hl) = if headers.is_empty() { (null(), 0) } else { (head.as_ptr(), u32::MAX) };
        if WinHttpSendRequest(req.0, hp, hl, null(), 0, 0, 0) == 0 || WinHttpReceiveResponse(req.0, null_mut()) == 0 {
            return Err(offline());
        }
        let mut status: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            req.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            null(),
            &mut status as *mut u32 as *mut c_void,
            &mut size,
            null_mut(),
        );
        if status != 200 {
            return Err(format!("Le serveur a répondu {status}"));
        }
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let mut read: u32 = 0;
            if WinHttpReadData(req.0, buf.as_mut_ptr() as *mut c_void, buf.len() as u32, &mut read) == 0 {
                return Err("La connexion a été coupée".into());
            }
            if read == 0 {
                return Ok(());
            }
            sink(&buf[..read as usize])?;
        }
    }
}

/// Le contenu de `url` (une réponse courte : au plus 4 Mo).
pub fn get(url: &str, headers: &[&str]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    request(url, headers, 15, |chunk| {
        if out.len() + chunk.len() > 4 << 20 {
            return Err("Réponse trop longue".into());
        }
        out.extend_from_slice(chunk);
        Ok(())
    })?;
    Ok(out)
}

/// Télécharge `url` dans `path` et renvoie (taille, empreinte SHA-256 en hexadécimal).
pub fn download(url: &str, path: &Path) -> Result<(u64, String), String> {
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let done = request(url, &["User-Agent: Scriba"], 60, |chunk| {
        hash.update(chunk);
        size += chunk.len() as u64;
        file.write_all(chunk).map_err(|e| e.to_string())
    });
    if let Err(e) = done.and_then(|()| file.flush().map_err(|e| e.to_string())) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(e);
    }
    Ok((size, hex(&hash.finalize())))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empreinte() {
        assert_eq!(hex(&Sha256::digest(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn adresse_refusee() {
        assert!(get("http://example.com/", &[]).is_err());
    }

    /// Essai réel (réseau) : `cargo test --release reseau -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn reseau() {
        let body = get(
            "https://api.github.com/repos/Kaynegiordano/scriba/releases/latest",
            &["User-Agent: Scriba", "Accept: application/vnd.github+json"],
        )
        .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let tag = json["tag_name"].as_str().unwrap();
        println!("dernière publication : {tag}");
        let asset = json["assets"].as_array().unwrap().iter().find(|a| a["name"] == "Scriba.exe").unwrap();
        let tmp = std::env::temp_dir().join("scriba-net-test.bin");
        let (size, sha) = download(asset["browser_download_url"].as_str().unwrap(), &tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        println!("{size} octets, sha256 {sha}");
        assert_eq!(size, asset["size"].as_u64().unwrap());
        assert_eq!(Some(sha.as_str()), asset["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")));
    }
}
