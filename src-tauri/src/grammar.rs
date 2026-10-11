//! Vérification de la phrase en cours : orthographe, accords sujet-verbe,
//! -er / -é, homophones (a/à, et/est, son/sont…), accord après le déterminant.
//! Règles volontairement simples et expliquées comme en classe de 4e.

use crate::lexicon::{pronoun, Entry, Form, Hint, Lexicon, IND, PLUR, PRE, SING};

#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub lower: String,
    /// position (en caractères) dans la phrase
    pub start: usize,
    pub end: usize,
    /// numéro de phrase (coupée par . ! ?)
    pub sent: usize,
}

#[derive(Debug, Clone)]
pub struct Correction {
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub replacement: String,
    pub reason: String,
    pub kind: &'static str,
}

const CLITICS: &[&str] =
    &["ne", "n'", "me", "m'", "te", "t'", "se", "s'", "le", "la", "l'", "les", "lui", "leur", "y", "en", "nous", "vous"];
const ADVERBS: &[&str] = &[
    "pas", "bien", "déjà", "jamais", "rien", "tout", "encore", "beaucoup", "trop", "plus", "toujours", "souvent",
    "vraiment", "aussi", "mal", "enfin", "presque", "peu",
];
const PREPS: &[&str] = &["à", "de", "d'", "pour", "sans", "par"];
const MODALS: &[&str] = &[
    "aller", "pouvoir", "vouloir", "devoir", "savoir", "falloir", "faire", "laisser", "aimer", "adorer", "préférer",
    "détester", "espérer", "oser", "sembler", "venir", "souhaiter", "désirer", "compter", "croire", "penser",
];
const DET_PLUR: &[&str] =
    &["les", "des", "ces", "mes", "tes", "ses", "nos", "vos", "leurs", "plusieurs", "quelques", "aux"];
const DET_SING: &[&str] =
    &["le", "la", "un", "une", "ce", "cet", "cette", "mon", "ma", "ton", "ta", "son", "sa", "du", "au", "chaque"];

fn same_sound(a: &str, b: &str) -> bool {
    use crate::phonetics::{key, phonemes};
    key(&phonemes(a, true)) == key(&phonemes(b, true))
}

fn is_apostrophe(c: char) -> bool {
    c == '\'' || c == '’'
}

fn subject_of(word: &str) -> Option<u8> {
    match word {
        "je" | "j'" => Some(1),
        "tu" => Some(2),
        "il" | "elle" | "on" => Some(3),
        "nous" => Some(4),
        "vous" => Some(5),
        "ils" | "elles" => Some(6),
        _ => None,
    }
}

/// Découpe la phrase en mots. « j' », « l' » restent des mots ; « aujourd'hui »,
/// « peut-être » sont recollés s'ils existent dans le dictionnaire.
pub fn tokenize(lex: &Lexicon, chars: &[char]) -> Vec<Token> {
    let mut raw: Vec<Token> = Vec::new();
    let mut sent = 0;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_alphabetic() {
                i += 1;
            }
            if i < chars.len() && is_apostrophe(chars[i]) {
                i += 1;
            }
            let text: String = chars[start..i].iter().map(|c| if is_apostrophe(*c) { '\'' } else { *c }).collect();
            raw.push(Token { lower: text.to_lowercase(), text, start, end: i, sent });
            continue;
        }
        if matches!(c, '.' | '!' | '?' | ';' | ':') {
            sent += 1;
        }
        i += 1;
    }
    // recoller « aujourd'hui », « peut-être », « grand-mère »
    let mut out: Vec<Token> = Vec::with_capacity(raw.len());
    for t in raw {
        if let Some(prev) = out.last_mut() {
            let glued = prev.end == t.start && prev.text.ends_with('\'');
            let hyphen = prev.end + 1 == t.start && chars.get(prev.end) == Some(&'-');
            if glued || hyphen {
                let joined = if hyphen { format!("{}-{}", prev.text, t.text) } else { format!("{}{}", prev.text, t.text) };
                if lex.lookup(&joined).is_some() {
                    prev.lower = joined.to_lowercase();
                    prev.text = joined;
                    prev.end = t.end;
                    continue;
                }
            }
        }
        out.push(t);
    }
    out
}

/// Sujet « groupe du nom » en début de phrase : « Les chats | dorment », « Le petit chien | aboie ».
fn noun_subject(lex: &Lexicon, toks: &[Token], i: usize) -> Option<(u8, usize)> {
    let start = (0..=i).rev().take_while(|k| toks[*k].sent == toks[i].sent).last()?;
    let span = i.checked_sub(start)?;
    if !(2..=3).contains(&span) {
        return None;
    }
    let det = toks[start].lower.as_str();
    let n = if DET_PLUR.contains(&det) && det != "aux" {
        6
    } else if DET_SING.contains(&det) && !matches!(det, "du" | "au") {
        3
    } else {
        return None;
    };
    let noun = lex.lookup(&toks[i - 1].lower)?;
    let adj_ok = span == 2 || lex.lookup(&toks[start + 1].lower).is_some_and(|e| e.top_tag() == "ADJ");
    (noun.top_tag() == "NOM" && adj_ok).then_some((n, i - 1))
}

/// Personne du sujet pronom juste avant le mot `i` (en sautant ne, le, lui…).
/// « il nous parle » -> il ; « les enfants nous parlent » -> inconnu.
fn subject_before(toks: &[Token], i: usize) -> Option<(u8, usize)> {
    let mut found: Option<(u8, usize)> = None;
    let mut j = i;
    while j > 0 {
        j -= 1;
        let t = &toks[j];
        if t.sent != toks[i].sent {
            break;
        }
        if let Some(p) = subject_of(&t.lower) {
            found = Some((p, j));
            if t.lower != "nous" && t.lower != "vous" {
                return found;
            }
            continue;
        }
        if CLITICS.contains(&t.lower.as_str()) {
            continue;
        }
        // « nous / vous » précédé d'un autre mot : complément, sujet inconnu
        return if found.is_some_and(|(_, k)| toks[k].lower == "nous" || toks[k].lower == "vous") {
            None
        } else {
            found
        };
    }
    found
}

fn prev_index(toks: &[Token], i: usize, skip: &[&str]) -> Option<usize> {
    let mut j = i;
    while j > 0 {
        j -= 1;
        if toks[j].sent != toks[i].sent {
            return None;
        }
        if !skip.contains(&toks[j].lower.as_str()) {
            return Some(j);
        }
    }
    None
}

fn lemma_is(lex: &Lexicon, e: &Entry, names: &[&'static str]) -> Option<&'static str> {
    e.verbs.iter().find_map(|(l, _)| {
        let n = lex.lemma_name(*l);
        names.iter().find(|x| **x == n).copied()
    })
}

/// Auxiliaire (avoir / être) conjugué juste avant le mot `i`, avec un sujet.
fn aux_before<'a>(lex: &'a Lexicon, toks: &[Token], i: usize) -> Option<(&'static str, usize)> {
    let j = prev_index(toks, i, ADVERBS)?;
    let e = lex.lookup(&toks[j].lower)?;
    let aux = lemma_is(lex, e, &["avoir", "être"])?;
    if !e.is_finite() || subject_before(toks, j).is_none() {
        return None;
    }
    Some((aux, j))
}

/// Mot (préposition ou verbe comme aller, pouvoir…) qui demande un infinitif après lui.
fn wants_infinitive(lex: &Lexicon, toks: &[Token], i: usize) -> Option<usize> {
    let mut skip: Vec<&str> = CLITICS.to_vec();
    skip.push("pas");
    let j = prev_index(toks, i, &skip)?;
    let w = toks[j].lower.as_str();
    if PREPS.contains(&w) {
        return Some(j);
    }
    let e = lex.lookup(w)?;
    if matches!(e.top_tag(), "VER" | "AUX") && lemma_is(lex, e, MODALS).is_some() {
        return Some(j);
    }
    None
}

fn det_number(toks: &[Token], i: usize, lex: &Lexicon) -> Option<(u8, usize)> {
    let mut j = i.checked_sub(1)?;
    // un adjectif peut se glisser entre les deux : « les petits chats »
    if toks[j].sent == toks[i].sent && lex.lookup(&toks[j].lower).is_some_and(|e| e.top_tag() == "ADJ") && j > 0 {
        let k = j - 1;
        if DET_PLUR.contains(&toks[k].lower.as_str()) || DET_SING.contains(&toks[k].lower.as_str()) {
            j = k;
        }
    }
    if toks[j].sent != toks[i].sent {
        return None;
    }
    let w = toks[j].lower.as_str();
    if DET_PLUR.contains(&w) {
        Some((PLUR, j))
    } else if DET_SING.contains(&w) {
        Some((SING, j))
    } else {
        None
    }
}

/// Ce que la phrase attend au mot `i` (pour classer les propositions).
pub fn hint(lex: &Lexicon, toks: &[Token], i: usize) -> Hint {
    Hint {
        person: subject_before(toks, i).map(|(p, _)| p),
        participle: aux_before(lex, toks, i).is_some(),
        infinitive: wants_infinitive(lex, toks, i).is_some(),
        number: det_number(toks, i, lex).map(|(n, _)| n),
    }
}

fn match_case(model: &str, word: &str) -> String {
    let mut cs = model.chars();
    match cs.next() {
        Some(f) if f.is_uppercase() && model.chars().count() > 1 && cs.all(|c| !c.is_lowercase()) => word.to_uppercase(),
        Some(f) if f.is_uppercase() => {
            let mut w = word.chars();
            w.next().map(|c| c.to_uppercase().chain(w).collect()).unwrap_or_default()
        }
        _ => word.to_string(),
    }
}

fn ending_tip(p: u8, subject: &str) -> String {
    let rule = match p {
        1 => "le verbe se termine souvent par -e, -s ou -x",
        2 => "le verbe se termine par -s (ou -x)",
        3 => "pas de -s : le verbe se termine par -e, -t ou -d",
        4 => "le verbe se termine par -ons",
        5 => "le verbe se termine par -ez",
        _ => "le verbe se termine par -ent (ou -ont)",
    };
    format!("Avec « {subject} », {rule}.")
}

fn plural_of(lex: &Lexicon, w: &str) -> Option<String> {
    let mut c = vec![format!("{w}s"), format!("{w}x")];
    if let Some(stem) = w.strip_suffix("al").or_else(|| w.strip_suffix("ail")) {
        c.insert(0, format!("{stem}aux"));
    }
    c.into_iter().find(|x| lex.lookup(x).is_some_and(|e| e.gn & PLUR != 0))
}

fn singular_of(lex: &Lexicon, w: &str) -> Option<String> {
    let mut c = Vec::new();
    if let Some(stem) = w.strip_suffix("aux") {
        c.push(format!("{stem}al"));
    }
    if let Some(stem) = w.strip_suffix('s').or_else(|| w.strip_suffix('x')) {
        c.push(stem.to_string());
    }
    c.into_iter().find(|x| lex.lookup(x).is_some_and(|e| e.gn & SING != 0))
}

/// Participe passé d'un verbe en -er, accordé avec le sujet si l'auxiliaire est « être ».
fn participle(lex: &Lexicon, lemma: &str, aux: &str, subject: &str) -> Option<String> {
    let stem = lemma.strip_suffix("er")?;
    let ending = match (aux, subject) {
        ("être", "elle") => "ée",
        ("être", "elles") => "ées",
        ("être", "ils") => "és",
        _ => "é",
    };
    let p = format!("{stem}{ending}");
    lex.lookup(&p).map(|_| p)
}

/// Vérifie les mots terminés de la phrase (pas le mot en cours de frappe).
pub fn check(lex: &Lexicon, chars: &[char]) -> Vec<Correction> {
    let toks = tokenize(lex, chars);
    let mut out: Vec<Correction> = Vec::new();
    let done = |t: &Token| t.end < chars.len() && !chars[t.end].is_alphabetic();

    for (i, t) in toks.iter().enumerate() {
        if !done(t) || out.iter().any(|c| c.start == t.start) {
            continue;
        }
        let lower = t.lower.as_str();
        let next = toks.get(i + 1).filter(|n| n.sent == t.sent);
        let prev = if i > 0 && toks[i - 1].sent == t.sent { Some(toks[i - 1].lower.as_str()) } else { None };
        let first_of_sentence = i == 0 || toks[i - 1].sent != t.sent;
        let mut fix = |replacement: String, reason: String, kind: &'static str| {
            out.push(Correction {
                start: t.start,
                end: t.end,
                original: t.text.clone(),
                replacement: match_case(&t.text, &replacement),
                reason,
                kind,
            });
        };

        // 1. orthographe
        let Some(e) = lex.lookup(lower) else {
            if lower.ends_with('\'') || lower.chars().count() < 2 {
                continue;
            }
            let capital = t.text.chars().next().is_some_and(|c| c.is_uppercase());
            if capital && !first_of_sentence {
                continue; // nom propre
            }
            let accents = lex.same_letters(lower);
            if let Some(best) = accents.first() {
                fix(best.word.clone(), format!("Attention aux accents : « {} ».", best.word), "orthographe");
            } else if let Some(s) = lex.suggest(lower, &hint(lex, &toks, i), 1).first() {
                // en début de phrase, un prénom (« Paul ») n'est pas une faute :
                // on ne corrige que si la proposition se prononce pareil
                let sure = !capital || same_sound(lower, &s.word);
                if s.word != lower && sure {
                    fix(s.word.clone(), format!("Mot inconnu. Tu voulais écrire « {} » ?", s.word), "orthographe");
                }
            }
            continue;
        };

        // 2. homophones
        let subj = subject_before(&toks, i);
        let next_entry = next.and_then(|n| lex.lookup(&n.lower));
        match lower {
            "à" if subj.is_some_and(|(p, _)| p == 3) => {
                fix("a".into(), "« il a » : c'est le verbe avoir (on peut dire « il avait »), sans accent.".into(), "homophone");
                continue;
            }
            "a" if subj.is_none()
                && next_entry.is_some_and(|n| n.is_infinitive() && n.top_tag() == "VER")
                && i > 0
                && lex.lookup(&toks[i - 1].lower).is_some_and(|p| p.top_tag() == "VER" && p.is_finite()) =>
            {
                fix("à".into(), "On ne peut pas dire « avait » ici : c'est la préposition « à ».".into(), "homophone");
                continue;
            }
            "et" if matches!(prev, Some("il" | "elle" | "on" | "c'" | "ce"))
                && next_entry.is_some_and(|n| n.top_tag() == "ADJ" || n.is_participle()) =>
            {
                fix("est".into(), "On peut dire « il était » : c'est le verbe être, « est ».".into(), "homophone");
                continue;
            }
            "son" if matches!(prev, Some("ils" | "elles")) => {
                fix("sont".into(), "On peut dire « ils étaient » : c'est le verbe être, « sont ».".into(), "homophone");
                continue;
            }
            "on" if matches!(prev, Some("ils" | "elles")) => {
                fix("ont".into(), "On peut dire « ils avaient » : c'est le verbe avoir, « ont ».".into(), "homophone");
                continue;
            }
            "ce" if matches!(prev, Some("il" | "elle" | "on" | "ils" | "elles")) => {
                fix("se".into(), "Devant un verbe, on écrit « se » (se laver, se dire).".into(), "homophone");
                continue;
            }
            "c'" if matches!(prev, Some("il" | "elle" | "on")) => {
                fix("s'".into(), "« il s'est… » : c'est « se », le verbe est pronominal.".into(), "homophone");
                continue;
            }
            "sa" if next_entry.is_some_and(|n| n.top_tag() == "VER" && n.is_finite()) => {
                fix("ça".into(), "On peut dire « cela » : on écrit « ça ».".into(), "homophone");
                continue;
            }
            "leur" if next_entry.is_some_and(|n| n.top_tag() == "NOM" && n.gn != 0 && n.gn & SING == 0) => {
                fix("leurs".into(), "Devant un nom au pluriel, « leur » prend un -s.".into(), "accord");
                continue;
            }
            _ => {}
        }

        let verb_first = matches!(e.top_tag(), "VER" | "AUX");
        let lemma = e.lemma().map(|l| lex.lemma_name(l)).unwrap_or("");

        // 3. -er / -é après avoir ou être : « il a mangé »
        if verb_first && lemma.ends_with("er") && !e.is_participle() {
            if let Some((aux, j)) = aux_before(lex, &toks, i) {
                let subject = subject_before(&toks, j).map(|(_, k)| toks[k].lower.as_str()).unwrap_or("");
                if let Some(p) = participle(lex, lemma, aux, subject) {
                    let tip = if aux == "avoir" { " Astuce : remplace par « vendu »." } else { "" };
                    fix(p.clone(), format!("Après « {aux} », on met le participe passé : « {p} ».{tip}"), "verbe");
                    continue;
                }
            }
        }

        // 4. infinitif après « pour », « de », « il va »… : « pour manger »
        if verb_first && lemma.ends_with("er") && !e.is_infinitive() && aux_before(lex, &toks, i).is_none() {
            if let Some(j) = wants_infinitive(lex, &toks, i) {
                fix(
                    lemma.to_string(),
                    format!("Après « {} », on met l'infinitif : « {lemma} ». Astuce : remplace par « vendre ».", toks[j].text),
                    "verbe",
                );
                continue;
            }
        }

        // 5. accord sujet-verbe : « ils mangent », « les chats dorment »
        let noun_subj = if subj.is_none() && verb_first { noun_subject(lex, &toks, i) } else { None };
        if let Some((p, k)) = subj.or(noun_subj) {
            if e.is_finite() && !e.has_person(p) && (verb_first || i == k + 1) {
                let target = e
                    .verbs
                    .iter()
                    .flat_map(|(l, fs)| {
                        let mut fs: Vec<Form> = fs.iter().copied().filter(|f| f.finite()).collect();
                        fs.sort_by_key(|f| !(f.mode == IND && f.tense == PRE));
                        fs.into_iter().map(move |f| (*l, f))
                    })
                    .find_map(|(l, f)| lex.conj(l, Form { person: p, ..f }));
                if let Some(target) = target {
                    let tip = if noun_subj.is_some() {
                        let start = toks[..=k].iter().rposition(|t| t.sent != toks[i].sent).map_or(0, |x| x + 1);
                        let group: Vec<&str> = toks[start..=k].iter().map(|t| t.lower.as_str()).collect();
                        let who = if p == 6 { "ils" } else { "il" };
                        format!("« {} » = « {who} » : {}", group.join(" "), ending_tip(p, who).trim_start_matches(&format!("Avec « {who} », ")[..]))
                    } else {
                        let who = toks[k].lower.trim_end_matches('\'');
                        let who = if who == "j" { "je" } else { who };
                        ending_tip(p, if who.is_empty() { pronoun(p) } else { who })
                    };
                    fix(target.to_string(), tip, "accord");
                    continue;
                }
            }
        }

        // 6. accord avec le déterminant : « les chats », « un cheval »
        if matches!(e.top_tag(), "NOM" | "ADJ") && e.gn != 0 {
            if let Some((n, j)) = det_number(&toks, i, lex) {
                let det = toks[j].text.as_str();
                if n == PLUR && e.gn & PLUR == 0 {
                    if let Some(p) = plural_of(lex, lower) {
                        fix(p.clone(), format!("Après « {det} », on met le pluriel : « {p} »."), "accord");
                        continue;
                    }
                } else if n == SING && e.gn & SING == 0 {
                    if let Some(s) = singular_of(lex, lower) {
                        fix(s.clone(), format!("Après « {det} », on met le singulier : « {s} »."), "accord");
                        continue;
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexicon::tests::lex;

    fn fixes(s: &str) -> Vec<(String, String)> {
        let chars: Vec<char> = s.chars().collect();
        check(lex(), &chars).into_iter().map(|c| (c.original, c.replacement)).collect()
    }

    fn one(s: &str, from: &str, to: &str) -> bool {
        let f = fixes(s);
        let ok = f.iter().any(|(a, b)| a == from && b == to);
        println!("{s:<40} -> {f:?}");
        ok
    }

    #[test]
    fn corrections() {
        let cases = [
            ("ils mange une pomme.", "mange", "mangent"),
            ("tu mange bien ", "mange", "manges"),
            ("je manges ", "manges", "mange"),
            ("nous mangez ", "mangez", "mangeons"),
            ("il les mangent ", "mangent", "mange"),
            ("Elles parle fort ", "parle", "parlent"),
            ("il a manger ", "manger", "mangé"),
            ("j'ai mangez ", "mangez", "mangé"),
            ("elle est aller ", "aller", "allée"),
            ("pour mangé ", "mangé", "manger"),
            ("il va mangé ", "mangé", "manger"),
            ("il commence a manger ", "a", "à"),
            ("il à faim ", "à", "a"),
            ("il et grand ", "et", "est"),
            ("ils son partis ", "son", "sont"),
            ("ils on mangé ", "on", "ont"),
            ("il ce lave ", "ce", "se"),
            ("sa va ", "sa", "ça"),
            ("les chat ", "chat", "chats"),
            ("les cheval ", "cheval", "chevaux"),
            ("un chevaux ", "chevaux", "cheval"),
            ("la mezon ", "mezon", "maison"),
            ("une ecole ", "ecole", "école"),
            ("leur parents ", "leur", "leurs"),
            ("Mezon de campagne ", "Mezon", "Maison"),
            ("les chats dorme ", "dorme", "dorment"),
            ("Le petit chien aboient ", "aboient", "aboie"),
        ];
        let failed: Vec<&str> = cases.iter().filter(|(s, a, b)| !one(s, a, b)).map(|(s, _, _)| *s).collect();

        // phrases correctes : aucune alerte
        let clean = [
            "il nous parle ",
            "nous nous lavons ",
            "les enfants nous parlent ",
            "il a mangé une pomme ",
            "elle est partie ",
            "je vais manger ",
            "il commence à manger ",
            "aujourd'hui il fait beau ",
            "elle et moi ",
            "il a envie de manger ",
            "Paul mange une pomme ",
            "peut-être demain ",
            "le temps passe ",
            "vous êtes là ",
            "je suis content ",
            "c'est bien ",
            "ils ont mangé ",
            "on a gagné ",
            "les enfants jouent ",
            "le chat dort ",
            "les livres de mon frère sont ",
        ];
        let noisy: Vec<String> =
            clean.iter().filter_map(|s| { let f = fixes(s); (!f.is_empty()).then(|| format!("{s} -> {f:?}")) }).collect();
        assert!(failed.is_empty() && noisy.is_empty(), "manqués : {failed:?}\nfausses alertes : {noisy:#?}");
    }

    #[test]
    fn indices() {
        let chars: Vec<char> = "ils mang".chars().collect();
        let toks = tokenize(lex(), &chars);
        let h = hint(lex(), &toks, toks.len() - 1);
        assert_eq!(h.person, Some(6));
    }
}
