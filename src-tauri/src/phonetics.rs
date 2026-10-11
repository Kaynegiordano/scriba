//! Conversion approximative orthographe -> phonèmes (alphabet de Lexique 3),
//! pensée pour ce qu'écrit un élève « comme il entend », puis réduction en
//! « clé » qui fusionne les sons faciles à confondre (é/è/e/eu, o/au, in/un).

fn is_vowel(c: char) -> bool {
    matches!(
        c,
        'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'à' | 'â' | 'ä' | 'é' | 'è' | 'ê' | 'ë' | 'î' | 'ï'
            | 'ô' | 'ö' | 'û' | 'ù' | 'ü' | 'ÿ'
    )
}

fn is_front(c: Option<char>) -> bool {
    matches!(c, Some('e' | 'i' | 'y' | 'é' | 'è' | 'ê' | 'ë' | 'î' | 'ï'))
}

/// Phonèmes Lexique d'un mot tapé. `final_rules` applique les règles de fin de
/// mot (lettres muettes, -er/-ez/-et) : on calcule les deux variantes car on ne
/// sait pas si l'élève a fini son mot.
pub fn phonemes(word: &str, final_rules: bool) -> String {
    let mut s: Vec<char> = Vec::with_capacity(word.len());
    for c in word.to_lowercase().chars().filter(|c| c.is_alphabetic()) {
        match c {
            'œ' => s.extend(['o', 'e']),
            'æ' => s.extend(['a', 'e']),
            c => s.push(c),
        }
    }

    if final_rules && s.len() > 2 {
        let n = s.len();
        if (n > 3 && s.ends_with(&['e', 'r'])) || s.ends_with(&['e', 'z']) || s.ends_with(&['e', 'd']) {
            s.truncate(n - 2);
            s.push('é');
        } else if s.ends_with(&['e', 't']) {
            s.truncate(n - 2);
            s.push('è');
        } else {
            while s.len() > 2 && matches!(s.last(), Some('s' | 't' | 'd' | 'x' | 'z' | 'p')) {
                s.pop();
            }
            if s.len() > 2 && s.last() == Some(&'e') {
                s.pop();
            }
        }
    }

    let s = s;
    let n = s.len();
    let at = |k: usize| s.get(k).copied();
    let has = |i: usize, p: &str| p.chars().enumerate().all(|(j, c)| at(i + j) == Some(c));
    // Une voyelle nasale n'existe que si n/m n'est pas suivi d'une voyelle ou d'un autre n/m.
    let nasal = |k: usize| !at(k).is_some_and(|c| is_vowel(c) || c == 'n' || c == 'm');

    let mut out = String::with_capacity(n + 2);
    let mut i = 0;
    while i < n {
        let c = s[i];
        let prev = if i > 0 { at(i - 1) } else { None };
        let next = at(i + 1);

        // (motif, son, condition) testés du plus long au plus court
        let rule: Option<(&str, usize)> = if has(i, "eaux") {
            Some(("o", 4))
        } else if has(i, "eau") {
            Some(("o", 3))
        } else if has(i, "ouill") {
            Some(("uj", 5))
        } else if has(i, "euill") {
            Some(("2j", 5))
        } else if has(i, "aill") {
            Some(("aj", 4))
        } else if has(i, "eill") {
            Some(("Ej", 4))
        } else if has(i, "ouil") && i + 4 == n {
            Some(("uj", 4))
        } else if has(i, "euil") && i + 4 == n {
            Some(("2j", 4))
        } else if has(i, "ail") && i + 3 == n {
            Some(("aj", 3))
        } else if has(i, "eil") && i + 3 == n {
            Some(("Ej", 3))
        } else if has(i, "oeu") {
            Some(("2", 3))
        } else if has(i, "oin") && nasal(i + 3) {
            Some(("w5", 3))
        } else if has(i, "oi") || has(i, "oî") {
            Some(("wa", 2))
        } else if has(i, "oy") {
            Some(("waj", 2))
        } else if has(i, "ou") || has(i, "où") || has(i, "oû") {
            Some((if at(i + 2).is_some_and(is_vowel) { "w" } else { "u" }, 2))
        } else if (has(i, "ain") || has(i, "aim") || has(i, "ein") || has(i, "eim")) && nasal(i + 3) {
            Some(("5", 3))
        } else if has(i, "ai") || has(i, "aî") || has(i, "ei") || has(i, "eî") {
            Some(("E", 2))
        } else if has(i, "ay") {
            Some((if at(i + 2).is_some_and(is_vowel) { "Ej" } else { "E" }, 2))
        } else if has(i, "au") {
            Some(("o", 2))
        } else if has(i, "eu") || has(i, "eû") {
            Some(("2", 2))
        } else if has(i, "ien") && nasal(i + 3) {
            Some(("j5", 3))
        } else if (has(i, "an") || has(i, "am") || has(i, "en") || has(i, "em")) && nasal(i + 2) {
            Some(("@", 2))
        } else if (has(i, "in") || has(i, "im") || has(i, "yn") || has(i, "ym") || has(i, "în"))
            && nasal(i + 2)
        {
            Some(("5", 2))
        } else if (has(i, "un") || has(i, "um")) && nasal(i + 2) {
            Some(("1", 2))
        } else if (has(i, "on") || has(i, "om")) && nasal(i + 2) {
            Some(("§", 2))
        } else if has(i, "ill") {
            Some(("ij", 3))
        } else if has(i, "tion") && prev != Some('s') && i > 0 {
            Some(("sj§", 4))
        } else if has(i, "ch") {
            Some(("S", 2))
        } else if has(i, "ph") {
            Some(("f", 2))
        } else if has(i, "gn") {
            Some(("N", 2))
        } else if has(i, "th") {
            Some(("t", 2))
        } else if has(i, "qu") {
            Some(("k", 2))
        } else if has(i, "sc") && is_front(at(i + 2)) {
            Some(("s", 2))
        } else if has(i, "gu") && is_front(at(i + 2)) {
            Some(("g", 2))
        } else if has(i, "ge") && matches!(at(i + 2), Some('a' | 'o' | 'u' | 'â' | 'ô')) {
            Some(("Z", 2))
        } else if has(i, "cc") && is_front(at(i + 2)) {
            Some(("ks", 2))
        } else if has(i, "ck") || has(i, "cc") {
            Some(("k", 2))
        } else if has(i, "ui") {
            Some(("8", 1))
        } else {
            None
        };
        if let Some((ph, len)) = rule {
            out.push_str(ph);
            i += len;
            continue;
        }

        // consonne doublée -> une seule
        if !is_vowel(c) && next == Some(c) {
            i += 1;
            continue;
        }

        let ph: &str = match c {
            'a' | 'à' | 'â' | 'ä' => "a",
            'e' => "°",
            'é' => "e",
            'è' | 'ê' | 'ë' => "E",
            'i' | 'î' | 'ï' => {
                if next.is_some_and(is_vowel) { "j" } else { "i" }
            }
            'y' => {
                if prev.is_some_and(is_vowel) && next.is_some_and(is_vowel) { "j" } else { "i" }
            }
            'o' | 'ô' | 'ö' => "o",
            'u' | 'û' | 'ù' | 'ü' => "y",
            'c' => {
                if is_front(next) { "s" } else { "k" }
            }
            'ç' => "s",
            'g' => {
                if is_front(next) { "Z" } else { "g" }
            }
            'h' => "",
            'j' => "Z",
            'q' => "k",
            'r' => "R",
            's' => {
                if prev.is_some_and(is_vowel) && next.is_some_and(is_vowel) { "z" } else { "s" }
            }
            'x' => "ks",
            'b' => "b",
            'd' => "d",
            'f' => "f",
            'k' => "k",
            'l' => "l",
            'm' => "m",
            'n' => "n",
            'p' => "p",
            't' => "t",
            'v' => "v",
            'w' => "w",
            'z' => "z",
            _ => "",
        };
        out.push_str(ph);
        i += 1;
    }
    out
}

/// Clé tolérante : fusionne les sons proches et retire les répétitions.
pub fn key(phon: &str) -> String {
    let mut out = String::with_capacity(phon.len());
    let mut last = None;
    for c in phon.chars() {
        let m = match c {
            'e' | 'E' | '°' | '2' | '9' => 'E',
            'O' => 'o',
            '1' => '5',
            'G' => 'N',
            c => c,
        };
        if last != Some(m) {
            out.push(m);
        }
        last = Some(m);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(w: &str) -> String {
        key(&phonemes(w, true))
    }

    #[test]
    fn mots_courants() {
        assert_eq!(k("fonetik"), key("fonetik"));
        assert_eq!(k("mezon"), key("mEz§"));
        assert_eq!(k("maison"), key("mEz§"));
        assert_eq!(k("oiseau"), key("wazo"));
        assert_eq!(k("wazo"), key("wazo"));
        assert_eq!(k("travail"), key("tRavaj"));
        assert_eq!(k("fille"), key("fij"));
        assert_eq!(k("grands"), key("gR@"));
        assert_eq!(k("enfant"), key("@f@"));
        assert_eq!(k("bien"), key("bj5"));
        assert_eq!(k("pied"), key("pje"));
        assert_eq!(k("manger"), key("m@Ze"));
        assert_eq!(k("mangeait"), key("m@ZE"));
        assert_eq!(k("ospital"), key("ospital"));
        assert_eq!(k("nation"), key("nasj§"));
    }
}
