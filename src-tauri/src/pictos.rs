//! Pictogrammes ARASAAC à côté des mots proposés.
//! Pictogrammes : Sergio Palao, propriété du Gouvernement d'Aragon, ARASAAC (arasaac.org), CC BY-NC-SA.
//! Les images des mots courants sont dans Scriba ; pour les autres, la barre demande l'image à
//! arasaac.org par son numéro (le site ne voit jamais les mots tapés).

use crate::lexicon::Lexicon;
use std::collections::HashMap;
use std::sync::OnceLock;

const MAP_TSV: &str = include_str!("../data/pictos.tsv");
const PACK: &[u8] = include_bytes!("../data/pictos.bin");

fn map() -> &'static HashMap<&'static str, u32> {
    static M: OnceLock<HashMap<&'static str, u32>> = OnceLock::new();
    M.get_or_init(|| {
        MAP_TSV
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .filter_map(|(w, id)| id.parse().ok().map(|id| (w, id)))
            .collect()
    })
}

/// numéro -> (début, longueur) de l'image PNG dans PACK
fn index() -> &'static HashMap<u32, (usize, usize)> {
    static I: OnceLock<HashMap<u32, (usize, usize)>> = OnceLock::new();
    I.get_or_init(|| {
        let mut out = HashMap::new();
        let mut i = 0;
        while i + 8 <= PACK.len() {
            let id = u32::from_le_bytes(PACK[i..i + 4].try_into().unwrap());
            let len = u32::from_le_bytes(PACK[i + 4..i + 8].try_into().unwrap()) as usize;
            out.insert(id, (i + 8, len));
            i += 8 + len;
        }
        out
    })
}

/// Le pictogramme d'un mot : le mot lui-même, sinon son verbe à l'infinitif (« mangeait » -> manger),
/// sinon son singulier ou son masculin.
pub fn id_for(lex: &Lexicon, word: &str) -> Option<u32> {
    let w = word.to_lowercase();
    let m = map();
    if let Some(id) = m.get(w.as_str()) {
        return Some(*id);
    }
    if let Some(e) = lex.lookup(&w) {
        for (l, _) in &e.verbs {
            if let Some(id) = m.get(lex.lemma_name(*l)) {
                return Some(*id);
            }
        }
    }
    let bases = [w.strip_suffix('s'), w.strip_suffix('x'), w.strip_suffix("es"), w.strip_suffix('e')];
    let found = bases.into_iter().flatten().find_map(|base| m.get(base).copied());
    found
}

/// L'image (data URL) si elle est dans Scriba.
pub fn data_url(id: u32) -> Option<String> {
    let (start, len) = *index().get(&id)?;
    Some(format!("data:image/png;base64,{}", base64(&PACK[start..start + len])))
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexicon::tests::lex;

    #[test]
    fn pictogrammes() {
        let l = lex();
        let maison = id_for(l, "maison").unwrap();
        assert_eq!(id_for(l, "maisons"), Some(maison));
        assert_eq!(id_for(l, "mangeaient"), id_for(l, "manger"));
        assert!(data_url(maison).unwrap().starts_with("data:image/png;base64,iVBOR"));
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
    }
}
