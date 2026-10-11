//! Dictionnaire (Lexique 3) et moteur de propositions : orthographique
//! (début de mot, accents ignorés) + phonétique (mot écrit « comme on l'entend »).

use crate::phonetics::{key, phonemes};
use serde::Serialize;
use std::collections::HashMap;

struct Entry {
    word: String,
    weight: f32,
    tag: &'static str,
    info: String,
}

#[derive(Clone, Serialize)]
pub struct Suggestion {
    pub word: String,
    /// catégorie courte : nom, adj., verbe…
    pub tag: &'static str,
    /// pour un verbe : « manger · imparfait (ils) »
    pub info: String,
}

pub struct Lexicon {
    entries: Vec<Entry>,
    by_ortho: Vec<(String, u32)>,
    by_phon: Vec<(String, u32)>,
}

pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.to_lowercase().chars() {
        match c {
            'à' | 'â' | 'ä' => out.push('a'),
            'é' | 'è' | 'ê' | 'ë' => out.push('e'),
            'î' | 'ï' => out.push('i'),
            'ô' | 'ö' => out.push('o'),
            'ù' | 'û' | 'ü' => out.push('u'),
            'ÿ' => out.push('y'),
            'ç' => out.push('c'),
            'œ' => out.push_str("oe"),
            'æ' => out.push_str("ae"),
            c => out.push(c),
        }
    }
    out
}

fn tag_label(tags: &str, is_verb: bool) -> &'static str {
    if is_verb {
        return "verbe";
    }
    match tags.split(',').next().unwrap_or("").split(':').next().unwrap_or("") {
        "NOM" => "nom",
        "ADJ" => "adj.",
        "ADV" => "adv.",
        "PRE" => "prép.",
        "CON" => "conj.",
        "ART" => "article",
        "PRO" => "pronom",
        "ONO" => "interj.",
        "VER" | "AUX" => "verbe",
        _ => "",
    }
}

fn person(p: &str) -> &'static str {
    match p {
        "1s" => "je",
        "2s" => "tu",
        "3s" => "il",
        "1p" => "nous",
        "2p" => "vous",
        "3p" => "ils",
        _ => "",
    }
}

/// « ind:pre:1s;ind:pre:3s;sub:pre:3s » -> « présent (je, il) »
fn verb_form(infover: &str) -> String {
    const MODES: [&str; 6] = ["ind", "cnd", "sub", "imp", "par", "inf"];
    let items: Vec<Vec<&str>> = infover.split(';').filter(|s| !s.is_empty()).map(|s| s.split(':').collect()).collect();
    let Some(mode) = MODES.iter().find(|m| items.iter().any(|it| it[0] == **m)) else {
        return String::new();
    };
    let tense = items.iter().find(|it| it[0] == *mode).and_then(|it| it.get(1).copied()).unwrap_or("");
    let persons: Vec<&str> = items
        .iter()
        .filter(|it| it[0] == *mode && it.get(1).copied() == Some(tense))
        .filter_map(|it| it.get(2).map(|p| person(p)))
        .filter(|p| !p.is_empty())
        .collect();
    let tense_name = match (*mode, tense) {
        ("inf", _) => "infinitif",
        ("par", "pas") => "participe passé",
        ("par", _) => "participe présent",
        ("imp", _) => "impératif",
        ("cnd", _) => "conditionnel",
        ("sub", "imp") => "subj. imparfait",
        ("sub", _) => "subjonctif",
        (_, "pre") => "présent",
        (_, "imp") => "imparfait",
        (_, "pas") => "passé simple",
        (_, "fut") => "futur",
        _ => "",
    };
    if persons.is_empty() {
        tense_name.to_string()
    } else {
        format!("{tense_name} ({})", persons.join(", "))
    }
}

fn range<'a>(v: &'a [(String, u32)], prefix: &str) -> &'a [(String, u32)] {
    let lo = v.partition_point(|(k, _)| k.as_str() < prefix);
    let hi = lo + v[lo..].partition_point(|(k, _)| k.starts_with(prefix));
    &v[lo..hi]
}

fn levenshtein1(a: &[u8], b: &[u8]) -> bool {
    // vrai si au plus une insertion / suppression / substitution
    let (a, b) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    if b.len() - a.len() > 1 {
        return false;
    }
    let mut i = 0;
    while i < a.len() && a[i] == b[i] {
        i += 1;
    }
    if i == a.len() {
        true
    } else if a.len() == b.len() {
        a[i + 1..] == b[i + 1..]
    } else {
        a[i..] == b[i + 1..]
    }
}

impl Lexicon {
    pub fn load(data: &str) -> Self {
        let mut entries = Vec::with_capacity(130_000);
        let mut by_ortho = Vec::with_capacity(130_000);
        let mut by_phon = Vec::with_capacity(140_000);
        for line in data.lines() {
            let mut f = line.split('\t');
            let (Some(word), Some(phons), Some(freq), Some(tags), lemma, infover) =
                (f.next(), f.next(), f.next(), f.next(), f.next().unwrap_or(""), f.next().unwrap_or(""))
            else {
                continue;
            };
            let idx = entries.len() as u32;
            let is_verb = !infover.is_empty() && tags.split(',').next().is_some_and(|t| t == "VER" || t == "AUX");
            let info = if is_verb {
                let form = verb_form(infover);
                if form == "infinitif" || lemma.is_empty() { form } else { format!("{lemma} · {form}") }
            } else {
                String::new()
            };
            entries.push(Entry {
                word: word.to_string(),
                weight: (1.0 + freq.parse::<f32>().unwrap_or(0.0)).ln(),
                tag: tag_label(tags, is_verb),
                info,
            });
            by_ortho.push((fold(word), idx));
            let mut seen: Vec<String> = Vec::new();
            for p in phons.split('|') {
                let k = key(p);
                if !seen.contains(&k) {
                    by_phon.push((k.clone(), idx));
                    seen.push(k);
                }
            }
        }
        by_ortho.sort_unstable();
        by_phon.sort_unstable();
        Lexicon { entries, by_ortho, by_phon }
    }

    pub fn suggest(&self, typed: &str, max: usize) -> Vec<Suggestion> {
        let folded = fold(typed);
        if folded.is_empty() {
            return Vec::new();
        }
        let mut score: HashMap<u32, f32> = HashMap::new();
        fn add(score: &mut HashMap<u32, f32>, idx: u32, s: f32) {
            let e = score.entry(idx).or_insert(f32::MIN);
            if s > *e {
                *e = s;
            }
        }

        // 1. début de mot (accents ignorés) ; le mot exact tapé passe en tête
        for (k, idx) in range(&self.by_ortho, &folded) {
            let w = self.entries[*idx as usize].weight;
            add(&mut score, *idx, w + if *k == folded { 6.0 } else { 0.0 });
        }

        // 2. phonétique : mots qui commencent par le même son
        //    (une seule lettre : seulement les mots qui se prononcent exactement ainsi, « o » -> au, eau)
        let letters = folded.chars().count();
        let mut keys = vec![key(&phonemes(typed, true)), key(&phonemes(typed, false))];
        keys.dedup();
        // Liaison entendue et écrite : « zenfan » (les enfants), « nami » (un ami)
        let mut liaison: Vec<String> = Vec::new();
        if letters >= 4 && folded.starts_with(['z', 'n', 't']) {
            liaison = keys.iter().map(|k| k.chars().skip(1).collect()).collect();
        }
        for (k, penalty) in keys.iter().map(|k| (k, 0.7)).chain(liaison.iter().map(|k| (k, 1.5))) {
            if k.is_empty() {
                continue;
            }
            let whole_only = letters < 2 || k.chars().count() < 2;
            for (pk, idx) in range(&self.by_phon, k) {
                if whole_only && pk != k {
                    continue;
                }
                let w = self.entries[*idx as usize].weight;
                add(&mut score, *idx, w - penalty + if pk == k { 2.5 } else { 0.0 });
            }
        }

        // 3. un son de travers toléré sur les mots entiers (« ospital » -> hôpital)
        if letters >= 4 && keys[0].len() >= 5 {
            let q = keys[0].as_bytes();
            for (pk, idx) in &self.by_phon {
                let p = pk.as_bytes();
                if p.first() == q.first() && p.len().abs_diff(q.len()) <= 1 && levenshtein1(p, q) {
                    add(&mut score, *idx, self.entries[*idx as usize].weight - 1.2);
                }
            }
        }

        let mut ranked: Vec<(u32, f32)> = score.into_iter().collect();
        ranked.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked
            .into_iter()
            .take(max)
            .map(|(idx, _)| {
                let e = &self.entries[idx as usize];
                Suggestion { word: e.word.clone(), tag: e.tag, info: e.info.clone() }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn lex() -> &'static Lexicon {
        static L: OnceLock<Lexicon> = OnceLock::new();
        L.get_or_init(|| Lexicon::load(include_str!("../data/lexique.tsv")))
    }

    fn words(t: &str) -> Vec<String> {
        lex().suggest(t, 6).into_iter().map(|s| s.word).collect()
    }

    #[test]
    fn propositions() {
        let mut failed = Vec::new();
        for (typed, want) in [
            ("fonetik", "phonétique"),
            ("mezon", "maison"),
            ("ospital", "hôpital"),
            ("wazo", "oiseau"),
            ("hopi", "hôpital"),
            ("ecol", "école"),
            ("mangé", "manger"),
            ("pe", "peut"),
            ("bocou", "beaucoup"),
            ("ojourdui", "aujourd'hui"),
            ("dificil", "difficile"),
            ("anglai", "anglais"),
            ("mintenan", "maintenant"),
            ("kelke", "quelque"),
            ("ecri", "écrit"),
            ("o", "au"),
            ("se", "ce"),
            ("mais", "mais"),
            ("ces", "ses"),
            ("zenfan", "enfants"),
            ("zami", "amis"),
            ("table", "table"),
        ] {
            let got = words(typed);
            println!("{typed:>10} -> {got:?}");
            if !got.iter().any(|w| w == want) {
                failed.push(typed);
            }
        }
        assert!(failed.is_empty(), "échecs : {failed:?}");

        // rapidité : une frappe doit rester bien sous les 10 ms
        let t = std::time::Instant::now();
        for w in ["a", "le", "mai", "ospi", "ospital", "mangeaient", "anticonstitutionnel"] {
            lex().suggest(w, 6);
        }
        let per = t.elapsed() / 7;
        println!("temps moyen par frappe : {per:?}");
        assert!(per.as_millis() < 10);
    }

    #[test]
    fn infos_verbes() {
        let s = lex().suggest("mangeaient", 1);
        assert_eq!(s[0].info, "manger · imparfait (ils)");
        assert_eq!(lex().suggest("à", 1)[0].tag, "prép.");
    }
}
