//! Mots anglais courants (data/english.txt) : un mot anglais n'est pas une faute de français,
//! Scriba ne le « corrige » pas et laisse aussi tranquilles les mots qui l'entourent.

use std::collections::HashSet;
use std::sync::OnceLock;

fn words() -> &'static HashSet<&'static str> {
    static W: OnceLock<HashSet<&'static str>> = OnceLock::new();
    W.get_or_init(|| {
        include_str!("../data/english.txt")
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect()
    })
}

/// Le mot (en minuscules) est un mot anglais courant.
pub fn is_word(w: &str) -> bool {
    words().contains(w)
}
