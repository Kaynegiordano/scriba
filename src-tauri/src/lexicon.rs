//! Dictionnaire (Lexique 3) et moteur de propositions : orthographique
//! (début de mot, accents ignorés) + phonétique (mot écrit « comme on l'entend »),
//! classées selon la grammaire de la phrase (après « ils » : mangent, etc.).

use crate::phonetics::{key, phonemes};
use serde::Serialize;
use std::collections::HashMap;

/// Genre-nombre (masque) : masculin/féminin × singulier/pluriel.
pub const MS: u8 = 1;
pub const MP: u8 = 2;
pub const FS: u8 = 4;
pub const FP: u8 = 8;
pub const SING: u8 = MS | FS;
pub const PLUR: u8 = MP | FP;

pub const IND: u8 = 0;
pub const CND: u8 = 1;
pub const SUB: u8 = 2;
pub const IMP: u8 = 3;
pub const PAR: u8 = 4;
pub const INF: u8 = 5;

pub const PRE: u8 = 0;
pub const IMPF: u8 = 1;
pub const PAS: u8 = 2;
pub const FUT: u8 = 3;

/// Une analyse verbale : mode, temps, personne (1..6 = je..ils, 0 = sans personne).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Form {
    pub mode: u8,
    pub tense: u8,
    pub person: u8,
}

impl Form {
    fn parse(s: &str) -> Option<Form> {
        let mut it = s.split(':');
        let mode = match it.next()? {
            "ind" => IND,
            "cnd" => CND,
            "sub" => SUB,
            "imp" => IMP,
            "par" => PAR,
            "inf" => INF,
            _ => return None,
        };
        let tense = match it.next().unwrap_or("") {
            "pre" | "" => PRE,
            "imp" => IMPF,
            "pas" => PAS,
            "fut" => FUT,
            _ => PRE,
        };
        let person = match it.next().unwrap_or("") {
            "1s" => 1,
            "2s" => 2,
            "3s" => 3,
            "1p" => 4,
            "2p" => 5,
            "3p" => 6,
            _ => 0,
        };
        Some(Form { mode, tense, person })
    }

    /// Forme conjuguée avec un sujet (indicatif, conditionnel, subjonctif).
    pub fn finite(&self) -> bool {
        self.mode <= SUB
    }
}

pub struct Entry {
    pub word: String,
    weight: f32,
    /// catégories Lexique, de la plus fréquente à la moins fréquente (« VER,NOM »)
    tags: Box<str>,
    pub gn: u8,
    /// (lemme, analyses), du lemme le plus fréquent au moins fréquent
    pub verbs: Vec<(u32, Vec<Form>)>,
    label: &'static str,
    info: String,
    /// clé de prononciation (pour trouver les homophones)
    phon: Box<str>,
    /// prononciation exacte (première variante de Lexique)
    pron: Box<str>,
    /// syllabes écrites (« mai-son »), vide si inconnues
    pub syll: Box<str>,
}

impl Entry {
    pub fn top_tag(&self) -> &str {
        self.tags.split(',').next().unwrap_or("").split(':').next().unwrap_or("")
    }
    pub fn forms(&self) -> impl Iterator<Item = (u32, Form)> + '_ {
        self.verbs.iter().flat_map(|(l, fs)| fs.iter().map(move |f| (*l, *f)))
    }
    pub fn is_finite(&self) -> bool {
        self.forms().any(|(_, f)| f.finite())
    }
    pub fn has_person(&self, p: u8) -> bool {
        self.forms().any(|(_, f)| f.finite() && f.person == p)
    }
    pub fn is_infinitive(&self) -> bool {
        self.forms().any(|(_, f)| f.mode == INF)
    }
    pub fn is_participle(&self) -> bool {
        self.forms().any(|(_, f)| f.mode == PAR && f.tense == PAS)
    }
    pub fn lemma(&self) -> Option<u32> {
        self.verbs.first().map(|(l, _)| *l)
    }
    /// Une des catégories Lexique du mot (« NOM », « ADJ »…).
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.split(',').any(|t| t.split(':').next() == Some(tag))
    }
    /// Fréquence (logarithme).
    pub fn weight(&self) -> f32 {
        self.weight
    }
}

/// Lexique écrit « soeur », « coeur », « oeil » : on rétablit la ligature (sœur, cœur, œil).
fn ligature(word: &str) -> String {
    if !word.contains("oe") {
        return word.to_string();
    }
    let c: Vec<char> = word.chars().collect();
    let mut out = String::with_capacity(word.len());
    let mut i = 0;
    while i < c.len() {
        let start = i == 0 || !c[i - 1].is_alphabetic();
        if c[i] == 'o' && c.get(i + 1) == Some(&'e') && (start || matches!(c.get(i + 2), Some('u' | 'i'))) {
            out.push('œ');
            i += 2;
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

#[derive(Clone, Serialize)]
pub struct Suggestion {
    pub word: String,
    /// catégorie courte : nom, adj., verbe…
    pub tag: &'static str,
    /// pour un verbe : « manger · imparfait (ils) »
    pub info: String,
    /// numéro du pictogramme ARASAAC (rempli par la barre)
    pub picto: Option<u32>,
}

/// Ce que la phrase attend au mot en cours (calculé par `grammar::hint`).
#[derive(Default, Clone, Copy, Debug)]
pub struct Hint {
    /// personne du sujet (1..6)
    pub person: Option<u8>,
    /// après avoir / être : participe passé
    pub participle: bool,
    /// après « pour », « de », « il va »… : infinitif
    pub infinitive: bool,
    /// après un déterminant : genre-nombre demandé (masque MS/FS/MP/FP)
    pub number: Option<u8>,
    /// le sujet est un pronom (je, il…) : juste après lui, on attend un verbe
    pub pronoun: bool,
    /// le mot suivant est « à », « au », « en », « chez »… (« je vais à la plage »)
    pub to: bool,
}

pub struct Lexicon {
    entries: Vec<Entry>,
    by_ortho: Vec<(String, u32)>,
    by_phon: Vec<(String, u32)>,
    lemmas: Vec<String>,
    conj: HashMap<(u32, Form), u32>,
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

fn tag_label(top: &str, is_verb: bool) -> &'static str {
    if is_verb {
        return "verbe";
    }
    match top {
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

pub fn pronoun(p: u8) -> &'static str {
    match p {
        1 => "je",
        2 => "tu",
        3 => "il",
        4 => "nous",
        5 => "vous",
        6 => "ils",
        _ => "",
    }
}

/// [manger: ind:pre:1s, ind:pre:3s, sub:pre:3s] -> « présent (je, il) »
fn verb_form(forms: &[Form]) -> String {
    const MODES: [u8; 6] = [IND, CND, SUB, IMP, PAR, INF];
    let Some(mode) = MODES.iter().copied().find(|m| forms.iter().any(|f| f.mode == *m)) else {
        return String::new();
    };
    let tense = forms.iter().find(|f| f.mode == mode).map(|f| f.tense).unwrap_or(PRE);
    let mut persons: Vec<u8> =
        forms.iter().filter(|f| f.mode == mode && f.tense == tense && f.person > 0).map(|f| f.person).collect();
    persons.sort_unstable();
    persons.dedup();
    let tense_name = match (mode, tense) {
        (INF, _) => "infinitif",
        (PAR, PAS) => "participe passé",
        (PAR, _) => "participe présent",
        (IMP, _) => "impératif",
        (CND, _) => "conditionnel",
        (SUB, IMPF) => "subj. imparfait",
        (SUB, _) => "subjonctif",
        (_, PRE) => "présent",
        (_, IMPF) => "imparfait",
        (_, PAS) => "passé simple",
        (_, FUT) => "futur",
        _ => "",
    };
    if persons.is_empty() {
        tense_name.to_string()
    } else {
        let names: Vec<&str> = persons.iter().map(|p| pronoun(*p)).collect();
        format!("{tense_name} ({})", names.join(", "))
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

/// Les deux mots diffèrent d'une seule faute de frappe : lettre en trop, oubliée,
/// remplacée ou deux lettres inversées (bonojur / bonjour, emfant / enfant).
fn one_typo(a: &[u8], b: &[u8]) -> bool {
    let (la, lb) = (a.len(), b.len());
    if a == b || la.abs_diff(lb) > 1 {
        return false;
    }
    let mut i = 0;
    while i < la.min(lb) && a[i] == b[i] {
        i += 1;
    }
    if la == lb {
        a[i + 1..] == b[i + 1..] || (i + 1 < la && a[i] == b[i + 1] && a[i + 1] == b[i] && a[i + 2..] == b[i + 2..])
    } else if la < lb {
        a[i..] == b[i + 1..]
    } else {
        a[i + 1..] == b[i..]
    }
}

fn edit_distance(a: &[u8], b: &[u8]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

fn add(score: &mut HashMap<u32, f32>, idx: u32, s: f32) {
    let e = score.entry(idx).or_insert(f32::MIN);
    if s > *e {
        *e = s;
    }
}

impl Lexicon {
    pub fn load(data: &str) -> Self {
        Self::load_with(data, "")
    }

    /// `extra` : mots récents absents de Lexique (un par ligne, `#` = commentaire).
    pub fn load_with(data: &str, extra: &str) -> Self {
        let mut entries = Vec::with_capacity(130_000);
        let mut by_ortho = Vec::with_capacity(130_000);
        let mut by_phon = Vec::with_capacity(140_000);
        let mut lemmas: Vec<String> = Vec::new();
        let mut lemma_ids: HashMap<String, u32> = HashMap::new();
        let mut conj: HashMap<(u32, Form), u32> = HashMap::new();

        for line in data.lines() {
            let mut f = line.split('\t');
            let (Some(word), Some(phons), Some(freq), Some(tags)) = (f.next(), f.next(), f.next(), f.next()) else {
                continue;
            };
            let word = ligature(word);
            let word = word.as_str();
            let verbs_col = f.next().unwrap_or("");
            let gn = u8::from_str_radix(f.next().unwrap_or("0"), 16).unwrap_or(0);
            let syll = f.next().unwrap_or("");
            let idx = entries.len() as u32;
            let weight = (1.0 + freq.parse::<f32>().unwrap_or(0.0)).ln();

            let mut verbs = Vec::new();
            for part in verbs_col.split('|').filter(|p| !p.is_empty()) {
                let Some((lemma, items)) = part.split_once('=') else { continue };
                let id = *lemma_ids.entry(lemma.to_string()).or_insert_with(|| {
                    lemmas.push(lemma.to_string());
                    (lemmas.len() - 1) as u32
                });
                let forms: Vec<Form> = items.split(';').filter_map(Form::parse).collect();
                for form in &forms {
                    // forme la plus fréquente pour (lemme, temps, personne)
                    let keep = conj.get(&(id, *form)).is_none_or(|old| entries_weight(&entries, *old) < weight);
                    if keep {
                        conj.insert((id, *form), idx);
                    }
                }
                verbs.push((id, forms));
            }

            let top = tags.split(',').next().unwrap_or("").split(':').next().unwrap_or("");
            let is_verb = !verbs.is_empty() && (top == "VER" || top == "AUX");
            let info = match verbs.first() {
                Some((l, forms)) if is_verb => {
                    let form = verb_form(forms);
                    if form == "infinitif" { form } else { format!("{} · {form}", lemmas[*l as usize]) }
                }
                _ => String::new(),
            };
            entries.push(Entry {
                word: word.to_string(),
                weight,
                tags: tags.into(),
                gn,
                verbs,
                label: tag_label(top, is_verb),
                info,
                phon: key(phons.split('|').next().unwrap_or("")).into(),
                pron: phons.split('|').next().unwrap_or("").into(),
                syll: syll.into(),
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
        // mots récents : on ignore ceux que Lexique connaît déjà
        let mut extra_added = false;
        for line in extra.lines() {
            let w = line.trim().to_lowercase();
            if w.is_empty() || w.starts_with('#') {
                continue;
            }
            let known = range(&by_ortho, &fold(&w)).iter().any(|(_, i)| entries[*i as usize].word == w);
            if known {
                continue;
            }
            let idx = entries.len() as u32;
            entries.push(Entry {
                word: w.clone(),
                weight: 5.0f32.ln_1p(),
                tags: "NOM".into(),
                gn: SING | PLUR,
                verbs: Vec::new(),
                label: "nom",
                info: String::new(),
                phon: key(&phonemes(&w, true)).into(),
                pron: phonemes(&w, true).into(),
                syll: "".into(),
            });
            by_ortho.push((fold(&w), idx));
            by_phon.push((key(&phonemes(&w, true)), idx));
            extra_added = true;
        }
        if extra_added {
            by_ortho.sort_unstable();
        }
        by_phon.sort_unstable();
        Lexicon { entries, by_ortho, by_phon, lemmas, conj }
    }

    /// Le mot exact (minuscules), s'il existe.
    pub fn lookup(&self, word: &str) -> Option<&Entry> {
        let lower = word.to_lowercase();
        range(&self.by_ortho, &fold(&lower))
            .iter()
            .map(|(_, i)| &self.entries[*i as usize])
            .find(|e| e.word == lower)
    }

    /// Mots qui s'écrivent pareil sans les accents (ecole -> école).
    pub fn same_letters(&self, word: &str) -> Vec<&Entry> {
        let f = fold(word);
        range(&self.by_ortho, &f)
            .iter()
            .filter(|(k, _)| *k == f)
            .map(|(_, i)| &self.entries[*i as usize])
            .collect()
    }

    /// Mot tapé sans accents (« peche », « ca ») : le mot accentué que la phrase attend.
    /// Le booléen dit si le choix est assez net pour être appliqué sans demander.
    pub fn restore_accents(&self, typed: &str, hint: &Hint) -> Option<(String, bool)> {
        let mut scored: Vec<(&Entry, f32)> = self
            .same_letters(typed)
            .into_iter()
            .filter(|e| e.word != typed)
            .map(|e| {
                let mut s = e.weight + Self::adjust(e, hint);
                // juste après un sujet (« ça », « il »), un mot qui ne peut pas être un verbe est improbable
                if Self::verb_expected(hint) && e.verbs.is_empty() {
                    s -= 2.5;
                }
                // après « a », « est », « ont »… : un participe passé, pas un nom
                if hint.participle && e.verbs.is_empty() {
                    s -= 3.5;
                }
                (e, s)
            })
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored.dedup_by(|a, b| a.0.word == b.0.word);
        let best = scored.first()?;
        // « on sai » : « saï » (un singe) n'est pas un verbe, ce n'est pas l'accent qui manque
        if Self::verb_expected(hint) && best.0.verbs.is_empty() {
            return None;
        }
        let sure = scored.get(1).is_none_or(|next| best.1 - next.1 >= 3.0);
        Some((best.0.word.clone(), sure))
    }

    /// Juste après un pronom sujet (« je », « il »…), sans auxiliaire : un verbe conjugué est attendu.
    fn verb_expected(hint: &Hint) -> bool {
        hint.person.is_some() && hint.pronoun && !hint.participle && !hint.infinitive
    }

    /// Les mots qui se prononcent comme `e` (ces / sait / c'est…), du plus courant au moins courant.
    pub fn homophones<'a>(&'a self, e: &'a Entry) -> Vec<&'a Entry> {
        let mut v: Vec<&Entry> = range(&self.by_phon, &e.phon)
            .iter()
            .filter(|(k, _)| **k == *e.phon)
            .map(|(_, i)| &self.entries[*i as usize])
            .filter(|h| h.word != e.word)
            .collect();
        v.sort_by(|a, b| b.weight.total_cmp(&a.weight));
        v.dedup_by(|a, b| a.word == b.word);
        v
    }

    /// Syllabes d'un mot (« maison » -> mai, son) ; le mot entier si inconnu.
    pub fn syllables(&self, word: &str) -> Vec<String> {
        let lower = word.to_lowercase();
        match self.lookup(&lower) {
            Some(e) if !e.syll.is_empty() => {
                // on recopie les lettres du mot tel qu'il est écrit (majuscules comprises)
                let mut out = Vec::new();
                let mut chars = word.chars();
                for part in e.syll.split('-') {
                    let n = part.chars().count();
                    let piece: String = chars.by_ref().take(n).collect();
                    if !piece.is_empty() {
                        out.push(piece);
                    }
                }
                let rest: String = chars.collect();
                if !rest.is_empty() {
                    match out.last_mut() {
                        Some(last) => last.push_str(&rest),
                        None => out.push(rest),
                    }
                }
                out
            }
            _ => vec![word.to_string()],
        }
    }

    pub fn lemma_name(&self, id: u32) -> &str {
        &self.lemmas[id as usize]
    }

    pub fn conj(&self, lemma: u32, form: Form) -> Option<&str> {
        self.conj.get(&(lemma, form)).map(|i| self.entries[*i as usize].word.as_str())
    }

    fn adjust(e: &Entry, hint: &Hint) -> f32 {
        let mut s = 0.0;
        let verb_first = matches!(e.top_tag(), "VER" | "AUX");
        if let Some(p) = hint.person {
            // juste après « ils » : un verbe conjugué à la bonne personne, pas un infinitif
            if e.has_person(p) {
                s += 2.0;
            } else if !e.verbs.is_empty() && !hint.participle && !hint.infinitive {
                s -= 4.0;
            }
        }
        if hint.participle {
            if e.is_participle() {
                s += 2.0;
            } else if verb_first {
                s -= 3.0;
            }
        }
        if hint.infinitive {
            if e.is_infinitive() {
                s += 2.0;
            } else if verb_first && (e.is_participle() || e.is_finite()) {
                s -= 3.0;
            }
        }
        if hint.to && ALLER.contains(&&*e.word) {
            s += 3.0;
        }
        if let Some(n) = hint.number {
            if matches!(e.top_tag(), "NOM" | "ADJ") && e.gn != 0 {
                s += if e.gn & n != 0 { 0.8 } else { -1.5 };
            }
            // juste après « le », « la », « mon »… : un nom ou un adjectif, pas un verbe
            if hint.person.is_none() && !hint.infinitive {
                s += match e.top_tag() {
                    "NOM" | "ADJ" => 2.5,
                    "VER" | "AUX" => -2.0,
                    _ => 0.0,
                };
            }
        }
        s
    }

    /// Le mot que l'élève voulait écrire, quand on peut en être presque certain :
    /// un seul candidat, ou un candidat bien plus courant que les autres.
    /// Candidats : mêmes sons (mezon), ou une seule faute de lettre (bonojur).
    pub fn confident(&self, typed: &str, hint: &Hint) -> Option<String> {
        let folded = fold(typed);
        let letters = folded.chars().count();
        // un mot de deux lettres (« fé », « pe ») n'est corrigé que si la phrase attend un verbe (« je fé »)
        let expects_verb = hint.person.is_some() || hint.infinitive || hint.participle;
        if letters < 2 || (letters == 2 && !expects_verb) {
            return None;
        }
        let mut cands: HashMap<u32, f32> = HashMap::new();
        let exact = [phonemes(typed, true), phonemes(typed, false)];
        let mut keys = vec![key(&exact[0]), key(&exact[1])];
        keys.dedup();
        for k in &keys {
            for (pk, idx) in range(&self.by_phon, k) {
                if pk == k {
                    let e = &self.entries[*idx as usize];
                    // un mot de deux lettres garde sa première lettre (fé -> fais, pas ia -> ya)
                    if letters == 2 && e.word.chars().next().map(|c| fold(&c.to_string())) != folded.chars().next().map(|c| c.to_string()) {
                        continue;
                    }
                    // après « je », « il »… : un verbe est attendu, pas un nom
                    let noun_after_subject = Self::verb_expected(hint) && e.verbs.is_empty();
                    // même prononciation exacte (« pleu » : pleut, pas plaît)
                    let exact = exact.iter().any(|p| **p == *e.pron);
                    let s = e.weight + Self::adjust(e, hint) - if noun_after_subject { 2.5 } else { 0.0 }
                        + if exact { 2.0 } else { 0.0 };
                    let old = cands.entry(*idx).or_insert(f32::MIN);
                    *old = old.max(s);
                }
            }
        }
        if letters >= 7 {
            let q = folded.as_bytes();
            for (k, idx) in &self.by_ortho {
                // une faute de lettre compte moins qu'un mot qui se prononce pareil
                if k.as_bytes().first() == q.first() && one_typo(k.as_bytes(), q) && !cands.contains_key(idx) {
                    let e = &self.entries[*idx as usize];
                    cands.insert(*idx, e.weight + Self::adjust(e, hint) - 2.5);
                }
            }
        }
        let mut ranked: Vec<(u32, f32)> = cands.into_iter().collect();
        ranked.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let best = ranked.first()?;
        // un pluriel ou un féminin du même mot (maison / maisons) n'est pas un vrai concurrent
        let best_folded = fold(&self.entries[best.0 as usize].word);
        let rival = ranked
            .iter()
            .skip(1)
            .find(|(i, _)| !one_typo(fold(&self.entries[*i as usize].word).as_bytes(), best_folded.as_bytes()));
        if rival.is_some_and(|r| best.1 - r.1 < 2.0) {
            return None;
        }
        // dans la même famille (enfant / enfants), le plus proche de ce qui a été tapé
        let closest = ranked
            .iter()
            .filter(|(i, _)| {
                *i == best.0 || one_typo(fold(&self.entries[*i as usize].word).as_bytes(), best_folded.as_bytes())
            })
            .min_by(|a, b| {
                let ctx = |i: u32| Self::adjust(&self.entries[i as usize], hint);
                let dist = |i: u32| edit_distance(fold(&self.entries[i as usize].word).as_bytes(), folded.as_bytes());
                ctx(b.0).total_cmp(&ctx(a.0)).then(dist(a.0).cmp(&dist(b.0)))
            })?;
        Some(self.entries[closest.0 as usize].word.clone())
    }

    pub fn suggest(&self, typed: &str, hint: &Hint, max: usize) -> Vec<Suggestion> {
        self.rank(typed, hint, max)
            .into_iter()
            .take(max)
            .map(|(idx, _)| {
                let e = &self.entries[idx as usize];
                Suggestion { word: e.word.clone(), tag: e.label, info: e.info.clone(), picto: None }
            })
            .collect()
    }

    /// La meilleure proposition pour un mot inconnu, si elle est nettement devant les autres :
    /// assez sûre pour être appliquée sans demander.
    pub fn guess(&self, typed: &str, hint: &Hint) -> Option<String> {
        // en dessous de 7 lettres, l'argot, les marques et les prénoms ressemblent trop à de vrais mots
        if fold(typed).chars().count() < 7 {
            return None;
        }
        let ranked = self.rank(typed, hint, 4);
        let (best, score) = *ranked.first()?;
        let word = &self.entries[best as usize].word;
        let best_folded = fold(word);
        let keys = [key(&phonemes(typed, true)), key(&phonemes(typed, false))];
        let same_start = best_folded.chars().next() == fold(typed).chars().next();
        let phon = self.entries[best as usize].phon.as_bytes();
        if !same_start && !keys.iter().any(|k| k.as_bytes() == phon || levenshtein1(phon, k.as_bytes())) {
            return None;
        }
        // le pluriel ou le féminin du même mot n'est pas un concurrent
        let rival = ranked
            .iter()
            .skip(1)
            .find(|(i, _)| !one_typo(fold(&self.entries[*i as usize].word).as_bytes(), best_folded.as_bytes()));
        if rival.is_some_and(|r| score - r.1 < 1.5) {
            return None;
        }
        Some(word.clone())
    }

    fn rank(&self, typed: &str, hint: &Hint, max: usize) -> Vec<(u32, f32)> {
        let folded = fold(typed);
        if folded.is_empty() {
            return Vec::new();
        }
        let mut score: HashMap<u32, f32> = HashMap::new();

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

        // 3 bis. une lettre de travers, quand on n'a presque rien trouvé
        if letters >= 4 && score.len() < max * 2 {
            let q = folded.as_bytes();
            for (k, idx) in &self.by_ortho {
                if one_typo(k.as_bytes(), q) {
                    add(&mut score, *idx, self.entries[*idx as usize].weight - 1.0);
                }
            }
        }

        // 4. la grammaire de la phrase départage
        let mut ranked: Vec<(u32, f32)> = score
            .into_iter()
            .map(|(i, s)| (i, s + Self::adjust(&self.entries[i as usize], hint)))
            .collect();
        ranked.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let _ = max;
        ranked
    }
}

/// Formes du verbe aller : après « je » + un mot qui se prononce « vé », « à » derrière -> « vais ».
const ALLER: &[&str] = &[
    "vais", "vas", "va", "vont", "allons", "allez", "allais", "allait", "allaient", "irai", "iras", "ira", "irons",
    "irez", "iront", "aille", "ailles", "aillent", "allé", "allés", "allée", "allées", "aller",
];

fn entries_weight(entries: &[Entry], i: u32) -> f32 {
    entries[i as usize].weight
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::OnceLock;

    pub fn lex() -> &'static Lexicon {
        static L: OnceLock<Lexicon> = OnceLock::new();
        L.get_or_init(|| Lexicon::load_with(include_str!("../data/lexique.tsv"), include_str!("../data/extra.txt")))
    }

    fn words(t: &str) -> Vec<String> {
        lex().suggest(t, &Hint::default(), 6).into_iter().map(|s| s.word).collect()
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
            lex().suggest(w, &Hint::default(), 6);
        }
        let per = t.elapsed() / 7;
        println!("temps moyen par frappe : {per:?}");
        assert!(per.as_millis() < 10);
    }

    #[test]
    fn infos_verbes() {
        let s = lex().suggest("mangeaient", &Hint::default(), 1);
        assert_eq!(s[0].info, "manger · imparfait (ils)");
        assert_eq!(lex().suggest("à", &Hint::default(), 1)[0].tag, "prép.");
        let manger = lex().lookup("manger").unwrap().lemma().unwrap();
        assert_eq!(lex().conj(manger, Form { mode: IND, tense: PRE, person: 6 }), Some("mangent"));
    }

    #[test]
    fn propositions_selon_la_phrase() {
        let first = |t: &str, h: Hint| lex().suggest(t, &h, 6)[0].word.clone();
        assert_eq!(first("mang", Hint { person: Some(6), ..Default::default() }), "mangent");
        assert_eq!(first("mang", Hint { person: Some(4), ..Default::default() }), "mangeons");
        assert_eq!(first("mang", Hint { participle: true, ..Default::default() }), "mangé");
        assert_eq!(first("mang", Hint { infinitive: true, ..Default::default() }), "manger");
        assert_eq!(first("cheva", Hint { number: Some(PLUR), ..Default::default() }), "chevaux");
        assert_eq!(first("cheva", Hint { number: Some(SING), ..Default::default() }), "cheval");
    }

}

