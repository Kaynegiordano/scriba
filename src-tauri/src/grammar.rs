//! Vérification de la phrase en cours : orthographe, accords sujet-verbe,
//! -er / -é, homophones (a/à, et/est, son/sont…), accord après le déterminant.
//! Règles volontairement simples et expliquées comme en classe de 4e.

use crate::custom;
use crate::lexicon::{fold, pronoun, Entry, Form, Hint, Lexicon, FP, FS, IND, IMPF, MP, MS, PLUR, PRE, SING};

#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub lower: String,
    /// position (en caractères) dans la phrase
    pub start: usize,
    pub end: usize,
    /// numéro de phrase (coupée par . ! ?)
    pub sent: usize,
    /// numéro de proposition (coupée aussi par la virgule, les parenthèses, les guillemets)
    pub clause: usize,
}

#[derive(Debug, Clone)]
pub struct Correction {
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub replacement: String,
    pub reason: String,
    pub kind: &'static str,
    /// assez sûre pour être appliquée toute seule
    pub auto: bool,
}

const CLITICS: &[&str] =
    &["ne", "n'", "me", "m'", "te", "t'", "se", "s'", "le", "la", "l'", "les", "lui", "leur", "y", "en", "nous", "vous"];
const ADVERBS: &[&str] = &[
    "pas", "bien", "déjà", "jamais", "rien", "tout", "encore", "beaucoup", "trop", "plus", "toujours", "souvent",
    "vraiment", "aussi", "mal", "enfin", "presque", "peu", "très", "si", "tellement", "assez", "super",
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
/// Nombres qui demandent le pluriel (« deux chats ») ; pas « neuf » (adjectif), ni « cent » (sent).
const NUMBERS: &[&str] = &[
    "deux", "trois", "quatre", "cinq", "six", "sept", "huit", "dix", "onze", "douze", "treize", "quatorze", "quinze",
    "seize", "vingt", "trente", "quarante", "cinquante", "soixante",
];
/// Mots qui ouvrent une nouvelle proposition : ce qui suit a son propre sujet.
const CLAUSE_WORDS: &[&str] = &[
    "et", "ou", "mais", "que", "qu'", "qui", "quand", "car", "donc", "si", "comme", "lorsque", "lorsqu'", "puisque",
    "puisqu'", "où", "dont", "parce", "alors", "puis", "ensuite", "enfin", "après", "avant",
];

fn det_plur(w: &str) -> bool {
    DET_PLUR.contains(&w) || NUMBERS.contains(&w)
}

/// Genre-nombre que demande un déterminant (masque MS/FS/MP/FP).
fn det_gn(w: &str) -> Option<u8> {
    Some(match w {
        "le" | "un" | "ce" | "cet" | "du" | "au" => MS,
        // « mon amie » : mon / ton / son vont aussi devant un nom féminin qui commence par une voyelle
        "mon" | "ton" | "son" => SING,
        "la" | "une" | "cette" | "ma" | "ta" | "sa" => FS,
        "l'" | "notre" | "votre" | "leur" | "chaque" => SING,
        w if det_plur(w) => PLUR,
        _ => return None,
    })
}

fn starts_with_vowel(w: &str) -> bool {
    w.starts_with(['a', 'e', 'é', 'è', 'ê', 'i', 'î', 'o', 'ô', 'u', 'y', 'â', 'œ'])
}

/// Premier mot de la proposition qui contient `i` (après « et », « quand », « parce que »…).
fn clause_start(toks: &[Token], i: usize) -> usize {
    let mut s = i;
    while s > 0 && toks[s - 1].clause == toks[i].clause && !CLAUSE_WORDS.contains(&toks[s - 1].lower.as_str()) {
        s -= 1;
    }
    s
}

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
        "il" | "elle" | "on" | "ça" => Some(3),
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
    let mut clause = 0;
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
            raw.push(Token { lower: text.to_lowercase(), text, start, end: i, sent, clause });
            continue;
        }
        if matches!(c, '.' | '!' | '?' | ';' | ':') {
            sent += 1;
        }
        if matches!(c, '.' | '!' | '?' | ';' | ':' | ',' | '(' | ')' | '"' | '«' | '»' | '\n') {
            clause += 1;
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

/// Sujet « groupe du nom » en début de proposition : « Les chats | dorment », « Le petit chien | aboie ».
fn noun_subject(lex: &Lexicon, toks: &[Token], i: usize) -> Option<(u8, usize)> {
    noun_group(lex, toks, i).map(|(p, k, _)| (p, k))
}

/// Les genres possibles d'un mot (MS|MP pour le masculin, FS|FP pour le féminin).
fn genders(gn: u8) -> u8 {
    (if gn & (MS | MP) != 0 { MS | MP } else { 0 }) | (if gn & (FS | FP) != 0 { FS | FP } else { 0 })
}

/// Le groupe du nom qui ouvre la proposition et finit juste avant `i` (« le petit chat », « la porte »,
/// « le chat noir ») : (personne, indice du nom, genre-nombre du groupe).
fn noun_group(lex: &Lexicon, toks: &[Token], i: usize) -> Option<(u8, usize, u8)> {
    let start = clause_start(toks, i);
    let span = i.checked_sub(start)?;
    if !(2..=3).contains(&span) {
        return None;
    }
    let det = toks[start].lower.as_str();
    if matches!(det, "du" | "au" | "aux") {
        return None;
    }
    let dmask = det_gn(det)?;
    let adj = |k: usize| lex.lookup(&toks[k].lower).is_some_and(|e| e.top_tag() == "ADJ");
    let k = match span {
        2 => start + 1,
        _ if adj(start + 1) => start + 2,
        _ if adj(start + 2) => start + 1,
        _ => return None,
    };
    let noun = lex.lookup(&toks[k].lower)?;
    if noun.top_tag() != "NOM" {
        return None;
    }
    let gn = match dmask & genders(noun.gn) {
        0 => dmask,
        g => g,
    };
    Some((if dmask & SING == 0 { 6 } else { 3 }, k, gn))
}

/// Genre-nombre du sujet du verbe être / avoir placé en `j` (« elle », « les filles », « nous »).
fn subject_gn(lex: &Lexicon, toks: &[Token], j: usize) -> Option<(u8, String)> {
    if let Some((_, k)) = subject_before(toks, j) {
        let w = toks[k].lower.as_str();
        let gn = match w {
            "elle" => FS,
            "elles" => FP,
            "ils" => MP,
            "nous" => PLUR,
            _ => return None,
        };
        return Some((gn, w.to_string()));
    }
    let (_, k, gn) = noun_group(lex, toks, j)?;
    let start = clause_start(toks, j);
    let words: Vec<&str> = toks[start..=k].iter().map(|t| t.lower.as_str()).collect();
    Some((gn, words.join(" ")))
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
    if !e.is_finite() || (subject_before(toks, j).is_none() && noun_subject(lex, toks, j).is_none()) {
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
        // « à la console », « de la tarte » : « la » est un article, le mot peut être un nom
        let article = j + 1 < i && matches!(toks[j + 1].lower.as_str(), "le" | "la" | "les" | "l'");
        if article && lex.lookup(&toks[i].lower).is_some_and(|e| e.has_tag("NOM")) {
            return None;
        }
        return Some(j);
    }
    let e = lex.lookup(w)?;
    if matches!(e.top_tag(), "VER" | "AUX") && lemma_is(lex, e, MODALS).is_some() {
        return Some(j);
    }
    None
}

/// Le déterminant devant le mot `i` (« les chats », « une grande maison ») : (genre-nombre demandé, indice).
fn det_number(toks: &[Token], i: usize, lex: &Lexicon) -> Option<(u8, usize)> {
    let mut j = i.checked_sub(1)?;
    // un adjectif peut se glisser entre les deux : « les petits chats »
    if toks[j].sent == toks[i].sent && lex.lookup(&toks[j].lower).is_some_and(|e| e.top_tag() == "ADJ") && j > 0 {
        let k = j - 1;
        if det_gn(&toks[k].lower).is_some() && toks[k].lower != "l'" {
            j = k;
        }
    }
    if toks[j].clause != toks[i].clause {
        return None;
    }
    let w = toks[j].lower.as_str();
    if !(det_plur(w) || DET_SING.contains(&w) || w == "l'") {
        return None;
    }
    let mut gn = det_gn(w)?;
    // « mon », « ton », « son » devant une consonne : masculin
    if matches!(w, "mon" | "ton" | "son") && j + 1 == i && !starts_with_vowel(&toks[i].lower) && !toks[i].lower.starts_with('h') {
        gn = MS;
    }
    Some((gn, j))
}

/// Ce que la phrase attend au mot `i` (pour classer les propositions).
pub fn hint(lex: &Lexicon, toks: &[Token], i: usize) -> Hint {
    let pronoun = subject_before(toks, i);
    // « les enfants mang… » -> mangent ; « les enfants qui jou… » -> jouent
    let group = if pronoun.is_none() { noun_subject(lex, toks, i).or_else(|| qui_subject(lex, toks, i)) } else { None };
    Hint {
        person: pronoun.or(group).map(|(p, _)| p),
        pronoun: pronoun.is_some(),
        participle: aux_before(lex, toks, i).is_some(),
        infinitive: wants_infinitive(lex, toks, i).is_some(),
        number: det_number(toks, i, lex).map(|(n, _)| n),
        to: toks
            .get(i + 1)
            .is_some_and(|n| n.sent == toks[i].sent && matches!(n.lower.as_str(), "à" | "a" | "au" | "aux" | "en" | "chez" | "dans")),
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

/// Mots qui ne se distinguent que par un accent : seul le sujet de la phrase permet de choisir.
/// (mot écrit sans accent, [(mot juste, indices du sens, écrits sans accent)])
type Cues = &'static [(&'static str, &'static [&'static str])];
const CUES: &[(&str, Cues)] = &[
    ("peche", &[
        ("pêche", &["poisson", "poissons", "canne", "mer", "riviere", "lac", "filet", "bateau", "truite", "saumon", "peche", "pecheur", "pecher",
                    "fruit", "fruits", "abricot", "nectarine", "tarte", "jus", "noyau", "peches", "dessert", "mange", "mangeons", "etang"]),
        ("pèche", &["ca", "problème", "probleme", "defaut", "erreur", "faute", "souci", "faiblesse", "ou"]),
        ("péché", &["eglise", "dieu", "confession", "pardon", "religion", "capital", "mignon", "mortel", "pretre", "cure", "diable", "enfer", "paradis", "ange", "bible"]),
    ]),
    ("tache", &[
        ("tache", &["vetement", "vetements", "chemise", "pull", "encre", "nettoyer", "enlever", "laver", "sale", "rousseur", "rousseurs", "soleil", "peau", "cafe", "sauce", "pantalon", "robe", "nappe", "detachant", "tee-shirt", "tshirt"]),
        ("tâche", &["devoir", "devoirs", "travail", "menagere", "menageres", "accomplir", "effectuer", "mission", "exercice", "projet", "difficile", "facile", "importante", "realiser", "terminer"]),
    ]),
    ("mur", &[
        ("mur", &["peinture", "brique", "briques", "maison", "escalier", "beton", "fissure", "grimpe", "affiche", "poster", "peindre"]),
        ("mûr", &["fruit", "fruits", "raisin", "tomate", "tomates", "banane", "cerise", "abricot", "melon", "pomme", "cueillir", "recolter", "arbre", "verger"]),
    ]),
    ("pate", &[
        ("pâte", &["pizza", "gateau", "tarte", "crepe", "crepes", "modeler", "farine", "dentifrice", "amande", "tartiner", "petrir", "cuisine", "recette"]),
        ("pâtes", &["spaghetti", "spaghettis", "cuire", "cuites", "nouilles", "diner", "dejeuner", "carbonara", "sauce", "bolognaise", "macaroni", "macaronis"]),
        ("pâté", &["foie", "campagne", "terrine", "viande", "tartine", "charcuterie", "croute"]),
    ]),
    ("pates", &[
        ("pâtes", &["spaghetti", "spaghettis", "cuire", "cuites", "nouilles", "diner", "dejeuner", "carbonara", "sauce", "bolognaise", "macaroni", "macaronis", "manger", "mange", "mangeons", "assiette", "fromage", "gratin", "tomate"]),
        ("pâtés", &["foie", "campagne", "terrine", "viande", "tartine", "charcuterie", "croute", "maison"]),
        ("pattes", &["chien", "chat", "animal", "animaux", "araignee", "poule", "oiseau", "griffes", "quatre", "arriere", "avant"]),
    ]),
    ("cote", &[
        ("côte", &["mer", "plage", "bretonne", "rotie", "roti", "porc", "boeuf", "os", "brisee", "cassee", "falaise", "ocean", "littoral", "bord"]),
        ("côté", &["a", "de", "du", "gauche", "droit", "droite", "autre", "cote", "dautre", "voisin", "coin", "pres"]),
    ]),
];

/// Le mot juste d'après les indices de la phrase (avant et après le mot) ; rien si les indices ne tranchent pas.
fn cue_choice(lex: &Lexicon, toks: &[Token], i: usize) -> Option<&'static str> {
    let plain = fold(&toks[i].lower);
    let (_, variants) = CUES.iter().find(|(p, _)| *p == plain)?;
    // on ne touche qu'à un mot écrit sans accent : un mot déjà accentué est présumé voulu
    if toks[i].lower != plain {
        return None;
    }
    let words: Vec<String> = toks
        .iter()
        .enumerate()
        .filter(|(j, t)| *j != i && t.sent == toks[i].sent)
        .map(|(_, t)| fold(t.lower.trim_end_matches('\'')))
        .collect();
    let mut scored: Vec<(&'static str, usize)> =
        variants.iter().map(|(v, cues)| (*v, words.iter().filter(|w| cues.contains(&w.as_str())).count())).collect();
    scored.sort_by_key(|s| std::cmp::Reverse(s.1));
    let (best, n) = scored[0];
    if n == 0 || scored.get(1).is_some_and(|s| s.1 == n) {
        return None;
    }
    // après « a », « est », « ont »… : le participe (« il a pêché un poisson »)
    if best == "pêche" && aux_before(lex, toks, i).is_some() {
        return Some("pêché");
    }
    Some(best)
}

/// « jai » -> « j'ai », « dorthograph » -> « d'orthographe » : l'apostrophe oubliée devant une voyelle.
fn missing_apostrophe(lex: &Lexicon, word: &str) -> Option<String> {
    // « quil » -> « qu'il », « quon » -> « qu'on »
    if let Some(rest) = word.strip_prefix("qu") {
        if starts_with_vowel(rest) && rest.chars().count() >= 2 {
            if let Some(e) = lex.lookup(rest).filter(|e| e.weight() >= 3.0) {
                return Some(format!("qu'{}", e.word));
            }
        }
    }
    let first = word.chars().next()?;
    if !matches!(first, 'd' | 'l' | 'j' | 'm' | 'n' | 's' | 't' | 'c') {
        return None;
    }
    let rest: String = word.chars().skip(1).collect();
    if !rest.starts_with(['a', 'e', 'é', 'è', 'ê', 'i', 'o', 'u', 'h', 'y', 'â', 'î', 'ô', 'û']) {
        return None;
    }
    let found = if let Some(e) = lex.lookup(&rest) {
        // « jai », « lami » : le reste doit être un vrai mot (pas un simple morceau)
        if rest.chars().count() < 2 { return None; }
        e.word.clone()
    } else if rest.chars().count() >= 4 {
        lex.confident(&rest, &Hint::default())?
    } else {
        return None;
    };
    // « j' », « m' », « t' », « s' », « n' », « c' » : seulement devant un verbe ou un pronom ; « l' », « d' » : partout
    if matches!(first, 'j' | 'm' | 't' | 's' | 'n' | 'c')
        && !lex.lookup(&found).is_some_and(|e| !e.verbs.is_empty() || e.top_tag() == "PRO")
    {
        return None;
    }
    Some(format!("{first}'{found}"))
}

/// Verbes suivis de la préposition « à » (« il est allé à l'école »).
const TO_VERBS: &[&str] = &[
    "aller", "venir", "arriver", "rentrer", "retourner", "passer", "penser", "jouer", "parler", "habiter", "rester",
    "répondre", "donner", "dire", "demander", "téléphoner", "ressembler", "tenir", "servir", "participer", "assister",
    "croire", "réfléchir", "renoncer", "appartenir", "obéir", "plaire", "manquer", "revenir", "monter", "descendre",
];
/// Verbes qui appellent « où » (« je vois où ça pèche », « dis-moi où il habite »).
const WHERE_VERBS: &[&str] = &[
    "voir", "savoir", "demander", "dire", "comprendre", "trouver", "chercher", "connaître", "regarder", "habiter",
    "aller", "venir", "mettre", "ranger", "poser", "vivre", "être", "retrouver", "deviner", "chercher", "indiquer",
    "montrer", "rester", "travailler",
];
/// Noms de lieu ou de temps suivis de « où » (« l'endroit où », « le jour où »).
const WHERE_NOUNS: &[&str] = &[
    "endroit", "jour", "moment", "lieu", "ville", "pays", "maison", "place", "côté", "là", "partout", "ailleurs",
    "part", "année", "soir", "matin", "époque", "école", "village", "quartier", "salle", "rue",
];
const SUBJECTS_AFTER_WHERE: &[&str] = &[
    "je", "j'", "tu", "il", "elle", "on", "nous", "vous", "ils", "elles", "ça", "ce", "c'", "le", "la", "les", "l'",
    "un", "une", "mon", "ma", "mes", "ton", "ta", "tes", "son", "sa", "ses", "notre", "votre", "leur", "ce", "cette",
];

/// « ou » est en fait « où » : suivi d'être / aller (« ou est mon sac »), ou placé après un verbe
/// ou un nom de lieu et avant un sujet (« je vois ou ça pèche », « l'endroit ou il habite »).
fn ou_is_where(lex: &Lexicon, toks: &[Token], i: usize) -> bool {
    let next = toks.get(i + 1).filter(|n| n.sent == toks[i].sent).map(|n| n.lower.as_str());
    if next.is_some_and(|n| {
        matches!(n, "est" | "sont" | "était" | "étaient" | "va" | "vont" | "allez" | "es" | "êtes" | "étais" | "vas")
    }) {
        return true;
    }
    if !next.is_some_and(|n| SUBJECTS_AFTER_WHERE.contains(&n)) {
        return false;
    }
    // le mot d'avant, en sautant « pas », « plus », « bien »…
    let mut j = i;
    while j > 0 {
        j -= 1;
        let p = &toks[j];
        if p.sent != toks[i].sent {
            return false;
        }
        if ADVERBS.contains(&p.lower.as_str()) {
            continue;
        }
        if WHERE_NOUNS.contains(&p.lower.as_str()) {
            return true;
        }
        return lex.lookup(&p.lower).is_some_and(|e| {
            e.verbs.iter().any(|(l, _)| WHERE_VERBS.contains(&lex.lemma_name(*l)))
        });
    }
    false
}

/// Après « ces » / « ses », ces mots montrent qu'il fallait « c'est » (« ces un chat », « ses pour toi »).
const C_EST_NEXT: &[&str] = &[
    "un", "une", "le", "la", "les", "l'", "des", "du", "mon", "ma", "ton", "ta", "son", "sa", "notre", "votre", "leur",
    "à", "au", "aux", "pour", "quoi", "qui", "moi", "toi", "lui", "eux", "ça", "comme", "vrai", "faux", "bien", "dur",
];
/// Après « mes », ces mots montrent qu'il fallait « mais » (« mes il est parti »).
const MAIS_NEXT: &[&str] = &[
    "je", "j'", "tu", "il", "elle", "on", "ils", "elles", "ça", "c'", "ce", "non", "oui", "pourquoi", "quand", "si",
];
/// Devant « foi », ces mots montrent qu'il fallait « fois » (« une fois », « chaque fois »).
const FOIS_BEFORE: &[&str] = &[
    "une", "chaque", "cette", "quelle", "première", "dernière", "deuxième", "troisième", "autre", "plusieurs", "des",
    "les", "deux", "trois", "quatre", "cinq", "dix", "cent", "mille", "quelques", "combien", "prochaine", "seule",
    "certaines", "toutes", "nombreuses",
];
/// Noms en -s qui ne sont pas des pluriels (« une fois », pas « une foi »).
const INVARIABLE: &[&str] = &["fois", "gens", "mœurs", "alentours", "environs"];

/// Mots collés que l'on écrit en plusieurs morceaux.
fn fused(w: &str) -> Option<&'static str> {
    Some(match w {
        "parceque" | "parcque" | "parsque" | "parske" | "parcke" => "parce que",
        "sava" | "çava" | "cava" => "ça va",
        "ilya" | "ilia" => "il y a",
        "ilyavait" | "iliavait" => "il y avait",
        "ilfaut" => "il faut",
        "biensur" | "biensûr" => "bien sûr",
        "tanpis" | "tempis" | "tantpis" => "tant pis",
        "peutetre" | "peutêtre" | "peutètre" | "peutettre" => "peut-être",
        "toutafait" | "toutàfait" => "tout à fait",
        "toutalheure" => "tout à l'heure",
        "toutdesuite" | "tousuite" | "toutsuite" | "toudsuite" => "tout de suite",
        "parcontre" => "par contre",
        "apeupres" | "apeuprès" => "à peu près",
        "quandmeme" | "quandmême" | "kanmeme" | "kanmême" => "quand même",
        "dutout" => "du tout",
        "enfaite" => "en fait",
        "atoutalheure" => "à tout à l'heure",
        "aujourdui" | "aujourdhui" | "ojourdui" | "ojourdhui" => "aujourd'hui",
        "quelquun" | "quelqun" | "kelkun" => "quelqu'un",
        "nimporte" => "n'importe",
        "cestadire" => "c'est-à-dire",
        _ => return None,
    })
}

/// Petits mots souvent collés au suivant (« lesenfants », « unami »).
const SPLIT_HEADS: &[&str] = &[
    "il", "je", "tu", "on", "ça", "de", "la", "le", "les", "un", "une", "en", "et", "au", "aux", "ne", "pas", "plus",
    "tout", "très", "trop", "pour", "par", "mon", "ma", "mes", "ton", "ta", "tes", "son", "sa", "ses", "des", "du",
    "dans", "sur", "avec", "sans", "chez", "bien", "mais", "que", "qui", "ce", "cette", "nos", "vos", "leur", "leurs",
];

/// « lesenfants » -> « les enfants » : un petit mot + un mot courant, et un seul découpage possible.
fn split_words(lex: &Lexicon, w: &str) -> Option<String> {
    if w.chars().count() < 5 {
        return None;
    }
    let mut found: Option<String> = None;
    for head in SPLIT_HEADS {
        let Some(rest) = w.strip_prefix(head) else { continue };
        if rest.chars().count() < 3 {
            continue;
        }
        // au moins ~20 fois par million de mots : pas un mot rare qui tomberait par hasard
        let Some(e) = lex.lookup(rest).filter(|e| e.weight() >= 3.0) else { continue };
        if found.is_some() {
            return None;
        }
        found = Some(format!("{head} {}", e.word));
    }
    found
}

/// La proposition a déjà un verbe conjugué avec son sujet avant le mot `i` (« il mange des pâtes | a »).
fn clause_has_verb(lex: &Lexicon, toks: &[Token], i: usize) -> bool {
    (clause_start(toks, i)..i).any(|j| {
        lex.lookup(&toks[j].lower).is_some_and(|e| e.is_finite() && matches!(e.top_tag(), "VER" | "AUX"))
            && subject_before(toks, j).is_some()
    })
}

/// « les enfants qui jouent », « c'est moi qui ai » : le sujet de « qui » est le mot d'avant.
fn qui_subject(lex: &Lexicon, toks: &[Token], i: usize) -> Option<(u8, usize)> {
    let j = prev_index(toks, i, CLITICS)?;
    if toks[j].lower != "qui" {
        return None;
    }
    let a = j.checked_sub(1)?;
    if toks[a].clause != toks[j].clause {
        return None;
    }
    let p = match toks[a].lower.as_str() {
        "moi" => 1,
        "toi" => 2,
        "lui" | "elle" => 3,
        "nous" => 4,
        "vous" => 5,
        "eux" | "elles" => 6,
        w => {
            let n = lex.lookup(w).filter(|n| n.top_tag() == "NOM")?;
            let (dm, _) = det_number(toks, a, lex)?;
            if dm & SING == 0 && n.gn & PLUR != 0 {
                6
            } else if dm & PLUR == 0 && n.gn & SING != 0 {
                3
            } else {
                return None;
            }
        }
    };
    Some((p, a))
}

/// « les enfants », « moi » : le groupe sujet, pour l'explication.
fn group_label(lex: &Lexicon, toks: &[Token], k: usize) -> String {
    match det_number(toks, k, lex) {
        Some((_, d)) if lex.lookup(&toks[k].lower).is_some_and(|e| e.top_tag() == "NOM") => {
            toks[d..=k].iter().map(|t| t.lower.as_str()).collect::<Vec<_>>().join(" ")
        }
        _ => toks[k].lower.clone(),
    }
}

/// « masculin pluriel », « féminin singulier »…
fn gn_name(need: u8) -> &'static str {
    let fem = need & (MS | MP) == 0;
    let masc = need & (FS | FP) == 0;
    match (fem, masc, need & SING == 0, need & PLUR == 0) {
        (true, _, true, _) => "féminin pluriel",
        (true, _, _, true) => "féminin singulier",
        (true, _, _, _) => "féminin",
        (_, true, true, _) => "masculin pluriel",
        (_, true, _, true) => "masculin singulier",
        (_, _, true, _) => "pluriel",
        _ => "singulier",
    }
}

/// Le déterminant `i` n'a pas le genre du nom qui suit : le bon déterminant, le nom, nom féminin ?
fn det_for_noun(lex: &Lexicon, toks: &[Token], i: usize) -> Option<(&'static str, String, bool)> {
    let det = toks[i].lower.as_str();
    if !matches!(det, "le" | "la" | "un" | "une" | "mon" | "ton" | "son" | "ma" | "ta" | "sa" | "ce" | "cet" | "cette") {
        return None;
    }
    // « il le porte », « je la vois » : ici « le », « la » sont des pronoms
    if matches!(det, "le" | "la") && subject_before(toks, i).is_some() {
        return None;
    }
    let first = toks.get(i + 1).filter(|n| n.clause == toks[i].clause)?;
    // c'est le mot juste après le déterminant qui décide de l'élision (« mon amie », « cet arbre »)
    let vowel = starts_with_vowel(&first.lower);
    let h = first.lower.starts_with('h');
    let mut k = i + 1;
    if lex.lookup(&first.lower).is_some_and(|e| e.top_tag() == "ADJ" && !e.has_tag("NOM")) {
        k += 1;
    }
    let n = toks.get(k).filter(|n| n.clause == toks[i].clause)?;
    let noun = lex.lookup(&n.lower)?;
    if noun.top_tag() != "NOM" || noun.gn & SING == 0 {
        return None;
    }
    let g = genders(noun.gn);
    let fem = g == FS | FP;
    let masc = g == MS | MP;
    if !fem && !masc {
        return None;
    }
    let target = match det {
        "le" if fem && !vowel && !h => "la",
        "la" if masc && !vowel && !h => "le",
        "un" if fem => "une",
        "une" if masc => "un",
        "mon" if fem && !vowel && !h => "ma",
        "ton" if fem && !vowel && !h => "ta",
        "son" if fem && !vowel && !h => "sa",
        "ma" if masc || vowel => "mon",
        "ta" if masc || vowel => "ton",
        "sa" if masc || vowel => "son",
        "ce" | "cet" if fem => "cette",
        "ce" if masc && vowel => "cet",
        "cet" if masc && !vowel && !h => "ce",
        "cette" if masc => {
            if vowel || h {
                "cet"
            } else {
                "ce"
            }
        }
        _ => return None,
    };
    Some((target, n.lower.clone(), fem))
}

/// L'adjectif `i` (avant ou après le nom) ne s'accorde pas avec son nom : (genre-nombre voulu, nom).
fn adj_agreement(lex: &Lexicon, toks: &[Token], i: usize, e: &Entry) -> Option<(u8, String)> {
    let is_adj = e.top_tag() == "ADJ" || (e.is_participle() && e.has_tag("ADJ"));
    if !is_adj || e.gn == 0 {
        return None;
    }
    let clause = toks[i].clause;
    let noun_at = |k: usize| {
        toks.get(k)
            .filter(|x| x.clause == clause)
            .and_then(|x| lex.lookup(&x.lower))
            .filter(|n| n.top_tag() == "NOM" && n.gn != 0)
    };
    let det_at = |k: usize| toks[k].clause == clause && det_gn(&toks[k].lower).is_some();
    let adj_at = |k: usize| toks[k].clause == clause && lex.lookup(&toks[k].lower).is_some_and(|x| x.top_tag() == "ADJ");
    // devant le nom : « une grande maison » ; après : « des chats noirs », « une petite robe verte »
    let (k, d) = if i >= 1 && det_at(i - 1) && noun_at(i + 1).is_some() {
        (i + 1, i - 1)
    } else if i >= 2 && noun_at(i - 1).is_some() && det_at(i - 2) {
        (i - 1, i - 2)
    } else if i >= 3 && noun_at(i - 1).is_some() && adj_at(i - 2) && det_at(i - 3) {
        (i - 1, i - 3)
    } else {
        return None;
    };
    let det = toks[d].lower.as_str();
    // « il les porte » : pronom, pas déterminant
    if matches!(det, "le" | "la" | "les" | "l'") && subject_before(toks, d).is_some() {
        return None;
    }
    let noun = noun_at(k)?;
    let dmask = det_gn(det)?;
    // déterminant et nom en désaccord (« une fois ») : on ne sait pas qui a raison
    if noun.gn & dmask == 0 {
        return None;
    }
    let g = match genders(noun.gn) {
        all if all == MS | MP | FS | FP => genders(dmask),
        g => g,
    };
    let num = if dmask & SING == 0 || noun.gn & SING == 0 {
        PLUR
    } else if dmask & PLUR == 0 || noun.gn & PLUR == 0 {
        SING
    } else {
        SING | PLUR
    };
    let need = g & num;
    if need == 0 || e.gn & need != 0 {
        return None;
    }
    Some((need, toks[k].lower.clone()))
}

/// « mangée » -> « mangé », « prises » -> « pris » : le participe au masculin singulier.
fn masculine_participle(lex: &Lexicon, w: &str) -> Option<String> {
    ["es", "e", "s"].iter().find_map(|suffix| {
        let m = w.strip_suffix(suffix)?;
        lex.lookup(m).filter(|e| e.is_participle() && e.gn & MS != 0).map(|e| e.word.clone())
    })
}

/// Vérifie les mots terminés de la phrase (pas le mot en cours de frappe).
pub fn check(lex: &Lexicon, chars: &[char]) -> Vec<Correction> {
    let mut toks = tokenize(lex, chars);
    let mut out: Vec<Correction> = Vec::new();
    let mut pre: Vec<Correction> = Vec::new();
    let done = |t: &Token| t.end < chars.len() && !chars[t.end].is_alphabetic();

    // 0. accents manquants : rétablis d'abord, de gauche à droite, pour que les mots suivants
    //    soient lus avec le bon contexte (« ca peche » -> « ça pèche », donc un verbe après « ça »)
    for i in 0..toks.len() {
        let lower = toks[i].lower.clone();
        // le sens de la phrase tranche entre « pêche », « pèche », « péché » (ou « tache » / « tâche »…)
        if let Some(word) = cue_choice(lex, &toks, i) {
            if word != lower {
                let t = &toks[i];
                let replacement = match_case(&t.text, word);
                if done(t) && !custom::ignored(&t.text, &replacement) {
                    pre.push(Correction {
                        start: t.start,
                        end: t.end,
                        original: t.text.clone(),
                        replacement,
                        reason: format!("D'après le sens de la phrase : « {word} »."),
                        kind: "orthographe",
                        auto: true,
                    });
                }
                toks[i].lower = word.to_string();
                continue;
            }
        }
        if lex.lookup(&lower).is_some() || lower.ends_with('\'') || lower.chars().count() < 2 || custom::knows(&lower) {
            continue;
        }
        let capital = toks[i].text.chars().next().is_some_and(|c| c.is_uppercase());
        let first = i == 0 || toks[i - 1].sent != toks[i].sent;
        if capital && !first {
            continue;
        }
        let Some((word, true)) = lex.restore_accents(&lower, &hint(lex, &toks, i)) else { continue };
        let t = &toks[i];
        let replacement = match_case(&t.text, &word);
        if done(t) && !custom::ignored(&t.text, &replacement) {
            pre.push(Correction {
                start: t.start,
                end: t.end,
                original: t.text.clone(),
                replacement,
                reason: format!("Attention aux accents : « {word} »."),
                kind: "orthographe",
                auto: true,
            });
        }
        toks[i].lower = word.to_lowercase();
    }

    for i in 0..toks.len() {
        // un mot déjà corrigé est relu avec sa correction (« le chat à mangé » : « a », puis « mangé » reste)
        if let Some(c) = out.last() {
            if let Some(k) = toks[..i].iter().rposition(|x| x.start == c.start) {
                toks[k].lower = c.replacement.to_lowercase();
            }
        }
        let t = toks[i].clone();
        let toks = &toks;
        if !done(&t) {
            continue;
        }
        let lower = t.lower.as_str();
        let next = toks.get(i + 1).filter(|n| n.sent == t.sent);
        let prev = if i > 0 && toks[i - 1].sent == t.sent { Some(toks[i - 1].lower.as_str()) } else { None };
        let first_of_sentence = i == 0 || toks[i - 1].sent != t.sent;
        // ce qui suit le mot : « il est la. » (fin de proposition) ou « la-bas »
        let after = chars.get(t.end).copied();
        let mut fix = |replacement: String, reason: String, kind: &'static str, auto: bool| {
            let replacement = match_case(&t.text, &replacement);
            if custom::ignored(&t.text, &replacement) || replacement == t.text {
                return;
            }
            out.push(Correction { start: t.start, end: t.end, original: t.text.clone(), replacement, reason, kind, auto });
        };

        // 1. orthographe
        let Some(e) = lex.lookup(lower) else {
            if lower.ends_with('\'') || lower.chars().count() < 2 || custom::knows(lower) {
                continue;
            }
            let capital = t.text.chars().next().is_some_and(|c| c.is_uppercase());
            if capital && !first_of_sentence {
                continue; // nom propre
            }
            let h = hint(lex, toks, i);
            if let Some(words) = fused(lower) {
                fix(words.into(), format!("Ce sont plusieurs mots : « {words} »."), "orthographe", true);
            } else if let Some((best, sure)) = lex.restore_accents(lower, &h) {
                fix(best.clone(), format!("Attention aux accents : « {best} »."), "orthographe", sure);
            } else if let Some(fixed) = missing_apostrophe(lex, lower) {
                fix(fixed.clone(), format!("Il manque une apostrophe : « {fixed} »."), "orthographe", true);
            } else if let Some(sure) = lex.confident(lower, &h) {
                fix(sure.clone(), format!("Mot inconnu. Tu voulais écrire « {sure} » ?"), "orthographe", true);
            } else if let Some(words) = split_words(lex, lower).filter(|_| !capital) {
                fix(words.clone(), format!("Il manque une espace : « {words} »."), "orthographe", true);
            } else if let Some(sure) = liaison(lex, prev, lower).and_then(|w| lex.confident(w, &h)) {
                fix(
                    sure.clone(),
                    format!("On entend la liaison, mais on ne l'écrit pas : « {} {sure} ».", prev.unwrap_or("")),
                    "orthographe",
                    true,
                );
            } else if let Some(s) = lex.suggest(lower, &h, 1).first() {
                // en début de phrase, un prénom (« Paul ») n'est pas une faute :
                // on ne corrige que si la proposition se prononce pareil
                let sure = !capital || same_sound(lower, &s.word);
                if s.word != lower && sure {
                    // proposition nettement devant les autres : appliquée sans demander
                    let clear = !capital && lex.guess(lower, &h).is_some_and(|g| g == s.word);
                    fix(s.word.clone(), format!("Mot inconnu. Tu voulais écrire « {} » ?", s.word), "orthographe", clear);
                }
            }
            continue;
        };

        // 2. homophones
        let subj = subject_before(toks, i);
        let noun_subj = noun_subject(lex, toks, i);
        let next_entry = next.and_then(|n| lex.lookup(&n.lower));
        let next_is_participle =
            next_entry.is_some_and(|n| matches!(n.top_tag(), "VER" | "AUX") && n.is_participle() && !n.is_infinitive());
        // le mot suivant en sautant les adverbes (« il et tres grand »)
        let next_word_entry = toks[i + 1..]
            .iter()
            .take_while(|n| n.sent == t.sent)
            .find(|n| !ADVERBS.contains(&n.lower.as_str()))
            .and_then(|n| lex.lookup(&n.lower));
        match lower {
            "a" if subj.is_none()
                && next.is_some()
                && i > 0
                && toks[i - 1].sent == t.sent
                && lex.lookup(&toks[i - 1].lower).is_some_and(|p| {
                    p.verbs.iter().any(|(l, _)| TO_VERBS.contains(&lex.lemma_name(*l)))
                }) =>
            {
                fix("à".into(), "Après ce verbe, c'est la préposition « à » (on ne peut pas dire « avait »).".into(), "homophone", true);
                continue;
            }
            "voila" => {
                fix("voilà".into(), "« voilà » s'écrit avec un accent grave.".into(), "orthographe", true);
                continue;
            }
            "sur" if {
                let before = if i > 0 && toks[i - 1].sent == t.sent { toks[i - 1].lower.as_str() } else { "" };
                let after = next.map(|n| n.lower.as_str());
                (before == "bien" && after.is_none_or(|n| SUBJECTS_AFTER_WHERE.contains(&n) || n == "que" || n == "qu'"))
                    || (matches!(before, "est" | "suis" | "es" | "sommes" | "êtes" | "sont" | "étais" | "était" | "pas")
                        && after.is_some_and(|n| matches!(n, "de" | "d'" | "que" | "qu'")))
            } =>
            {
                fix("sûr".into(), "« sûr » avec un accent circonflexe : on peut dire « certain ».".into(), "homophone", true);
                continue;
            }
            "ou" if ou_is_where(lex, toks, i) => {
                fix("où".into(), "« où » avec un accent : on peut dire « à quel endroit ».".into(), "homophone", true);
                continue;
            }
            "à" if subj.is_some_and(|(p, _)| p == 3) => {
                fix("a".into(), "« il a » : c'est le verbe avoir (on peut dire « il avait »), sans accent.".into(), "homophone", true);
                continue;
            }
            // « le chat à mangé » : devant un participe passé, c'est le verbe avoir
            "à" if next_is_participle && prev != Some("tout") => {
                fix("a".into(), "Devant « mangé », « fini »… c'est le verbe avoir (on peut dire « avait ») : « a ».".into(), "homophone", true);
                continue;
            }
            "a" if subj.is_none()
                && next_entry.is_some_and(|n| n.is_infinitive() && n.top_tag() == "VER")
                && i > 0
                && lex.lookup(&toks[i - 1].lower).is_some_and(|p| p.top_tag() == "VER" && p.is_finite()) =>
            {
                fix("à".into(), "On ne peut pas dire « avait » ici : c'est la préposition « à ».".into(), "homophone", true);
                continue;
            }
            // « il mange des pâtes a la sauce » : la proposition a déjà son verbe, ce « a » est « à »
            "a" if subj.is_none()
                && next.is_some()
                && !next_is_participle
                && noun_subj.is_none()
                && clause_has_verb(lex, toks, i) =>
            {
                fix("à".into(), "La phrase a déjà son verbe : ici, c'est la préposition « à » (on ne peut pas dire « avait »).".into(), "homophone", true);
                continue;
            }
            "et" if matches!(prev, Some("il" | "elle" | "on" | "c'" | "ce"))
                && next_word_entry.is_some_and(|n| n.top_tag() == "ADJ" || n.is_participle()) =>
            {
                fix("est".into(), "On peut dire « il était » : c'est le verbe être, « est ».".into(), "homophone", true);
                continue;
            }
            // « le ciel et bleu » : après un sujet, devant un adjectif seul
            "et" if noun_subj.is_some()
                && next_word_entry.is_some_and(|n| n.top_tag() == "ADJ")
                && toks.get(i + 2).filter(|x| x.clause == t.clause).is_none_or(|x| {
                    !lex.lookup(&x.lower).is_some_and(|e| e.top_tag() == "NOM")
                }) =>
            {
                fix("est".into(), "On peut dire « était » : c'est le verbe être, « est ».".into(), "homophone", true);
                continue;
            }
            "son" if matches!(prev, Some("ils" | "elles")) || noun_subj.is_some_and(|(p, _)| p == 6) => {
                fix("sont".into(), "On peut dire « ils étaient » : c'est le verbe être, « sont ».".into(), "homophone", true);
                continue;
            }
            "on" if matches!(prev, Some("ils" | "elles"))
                || (noun_subj.is_some_and(|(p, _)| p == 6)
                    && (next_is_participle
                        || next.is_some_and(|n| {
                            matches!(n.lower.as_str(), "faim" | "soif" | "peur" | "froid" | "chaud" | "raison" | "tort" | "besoin" | "envie" | "mal")
                        }))) =>
            {
                fix("ont".into(), "On peut dire « ils avaient » : c'est le verbe avoir, « ont ».".into(), "homophone", true);
                continue;
            }
            // « ont va au parc » : devant un verbe conjugué, c'est le pronom « on »
            "ont" if subj.is_none()
                && clause_start(toks, i) == i
                && next_entry.is_some_and(|n| matches!(n.top_tag(), "VER" | "AUX") && n.has_person(3) && !n.is_participle()) =>
            {
                fix("on".into(), "Devant un verbe, c'est le pronom « on » (on peut dire « il »).".into(), "homophone", true);
                continue;
            }
            // « il a mis sont manteau » : devant un nom, c'est le déterminant « son »
            "sont" if subj.is_none()
                && noun_subj.is_none()
                && !matches!(prev, Some("ce" | "c'" | "où" | "qui" | "que" | "qu'"))
                && next_entry.is_some_and(|n| n.top_tag() == "NOM" && n.verbs.is_empty() && n.gn & SING != 0 && n.gn & PLUR == 0) =>
            {
                fix("son".into(), "Devant un nom, c'est « son » (on peut dire « mon ») : son manteau.".into(), "homophone", true);
                continue;
            }
            "ce" if matches!(prev, Some("il" | "elle" | "on" | "ils" | "elles")) => {
                fix("se".into(), "Devant un verbe, on écrit « se » (se laver, se dire).".into(), "homophone", true);
                continue;
            }
            "c'" if matches!(prev, Some("il" | "elle" | "on")) => {
                fix("s'".into(), "« il s'est… » : c'est « se », le verbe est pronominal.".into(), "homophone", true);
                continue;
            }
            // « il ses levé » -> « il s'est levé »
            "ses" | "ces" | "sait" | "sais" if matches!(prev, Some("il" | "elle" | "on")) && next_is_participle => {
                fix("s'est".into(), "« il s'est levé » : c'est « se » + le verbe être.".into(), "homophone", true);
                continue;
            }
            // « tu tes trompé » -> « tu t'es trompé »
            "tes" if prev == Some("tu") => {
                fix("t'es".into(), "« tu t'es… » : c'est « te » + le verbe être.".into(), "homophone", true);
                continue;
            }
            // « ces un chat », « ses pour toi » -> « c'est »
            "ces" | "ses" if subj.is_none()
                && next.is_some_and(|n| {
                    C_EST_NEXT.contains(&n.lower.as_str()) || (n.lower == "pas" && clause_start(toks, i) == i)
                        // « ces beau » : un adjectif au singulier ne va pas avec « ces » (sauf « ses petit chats »)
                        || (next_entry.is_some_and(|x| x.top_tag() == "ADJ" && x.gn != 0 && x.gn & PLUR == 0)
                            && toks.get(i + 2).filter(|x| x.clause == t.clause).is_none_or(|x| {
                                !lex.lookup(&x.lower).is_some_and(|e| e.has_tag("NOM"))
                            }))
                }) =>
            {
                fix("c'est".into(), "On peut dire « cela est » : on écrit « c'est ».".into(), "homophone", true);
                continue;
            }
            // « mes il est parti » -> « mais »
            "mes" if next.is_some_and(|n| MAIS_NEXT.contains(&n.lower.as_str())) => {
                fix("mais".into(), "On peut dire « pourtant » : c'est « mais ».".into(), "homophone", true);
                continue;
            }
            // « il est la. » -> « là » ; « la-bas » -> « là-bas »
            "la" if (after == Some('-')
                && next.is_some_and(|n| matches!(n.lower.as_str(), "bas" | "haut" | "dessus" | "dessous" | "dedans")))
                || (matches!(after, Some('.' | '!' | '?' | ',' | ';' | ':'))
                    && prev.is_some_and(|p| {
                        matches!(p, "par" | "de" | "d'")
                            || lex.lookup(p).is_some_and(|e| e.is_finite() && lemma_is(lex, e, &["être"]).is_some())
                    })) =>
            {
                fix("là".into(), "« là » avec un accent : on peut dire « ici ».".into(), "homophone", true);
                continue;
            }
            "sa" if next_entry.is_some_and(|n| n.top_tag() == "VER" && n.is_finite()) => {
                fix("ça".into(), "On peut dire « cela » : on écrit « ça ».".into(), "homophone", true);
                continue;
            }
            // « ça maison » -> « sa maison »
            "ça" if next_entry.is_some_and(|n| {
                n.top_tag() == "NOM" && n.verbs.is_empty() && n.gn & FS != 0 && n.gn & (MS | MP | FP) == 0
            }) =>
            {
                fix("sa".into(), "Devant un nom, c'est « sa » (on peut dire « ma ») : sa maison.".into(), "homophone", true);
                continue;
            }
            "peu" if matches!(subj, Some((1..=3, _))) => {
                let (p, _) = subj.unwrap();
                let target = if p == 3 { "peut" } else { "peux" };
                fix(target.into(), format!("Avec « {} », c'est le verbe pouvoir : « {target} » (on peut dire « {} pouvait »).", pronoun(p), pronoun(p)), "homophone", true);
                continue;
            }
            "ma" | "ta" | "la"
                if next_entry.is_some_and(|n| n.top_tag() == "VER" && n.is_participle())
                    && matches!(prev, Some("il" | "elle" | "on" | "qui")) =>
            {
                let (target, who) = match lower {
                    "ma" => ("m'a", "me"),
                    "ta" => ("t'a", "te"),
                    _ => ("l'a", "le"),
                };
                fix(
                    target.into(),
                    format!("Devant un verbe, c'est « {who} » + « a » (avoir) : « {target} »."),
                    "homophone",
                    true,
                );
                continue;
            }
            "se" if next_entry.is_some_and(|n| n.top_tag() == "NOM" && n.verbs.is_empty() && n.gn & SING != 0) => {
                fix("ce".into(), "Devant un nom, on écrit « ce » (ce chat, ce matin).".into(), "homophone", true);
                continue;
            }
            "leur" if next_entry.is_some_and(|n| n.top_tag() == "NOM" && n.gn != 0 && n.gn & SING == 0) => {
                fix("leurs".into(), "Devant un nom au pluriel, « leur » prend un -s.".into(), "accord", true);
                continue;
            }
            // « il était une foi » -> « fois »
            "foi" if prev.is_some_and(|p| FOIS_BEFORE.contains(&p))
                || (prev == Some("la") && i > 1 && toks[i - 2].lower == "à") =>
            {
                fix("fois".into(), "« une fois » (on peut dire « une seule fois ») s'écrit avec un -s.".into(), "homophone", true);
                continue;
            }
            _ => {}
        }

        // 2 bis. le déterminant prend le genre du nom : « la maison », « un chien », « cet arbre »
        if let Some((target, noun, fem)) = det_for_noun(lex, toks, i) {
            let reason = if fem && matches!(target, "mon" | "ton" | "son" | "cet") {
                format!("Devant une voyelle, on écrit « {target} » : « {target} {noun} ».")
            } else {
                format!("« {noun} » est un nom {} : « {target} {noun} ».", if fem { "féminin" } else { "masculin" })
            };
            fix(target.into(), reason, "accord", true);
            continue;
        }

        let verb_first = matches!(e.top_tag(), "VER" | "AUX");
        let lemma = e.lemma().map(|l| lex.lemma_name(l)).unwrap_or("");

        // 3. -er / -é après avoir ou être : « il a mangé »
        if verb_first && lemma.ends_with("er") && !e.is_participle() {
            if let Some((aux, j)) = aux_before(lex, toks, i) {
                let subject = subject_before(toks, j).map(|(_, k)| toks[k].lower.as_str()).unwrap_or("");
                if let Some(p) = participle(lex, lemma, aux, subject) {
                    let tip = if aux == "avoir" { " Astuce : remplace par « vendu »." } else { "" };
                    fix(p.clone(), format!("Après « {aux} », on met le participe passé : « {p} ».{tip}"), "verbe", true);
                    continue;
                }
            }
        }

        // 4. infinitif après « pour », « de », « il va »… : « pour manger »
        if verb_first && lemma.ends_with("er") && !e.is_infinitive() && aux_before(lex, toks, i).is_none() {
            if let Some(j) = wants_infinitive(lex, toks, i) {
                // « à verse », « à pied » : après « à », un nom est possible, on ne fait que proposer
                let sure = !(e.has_tag("NOM") && PREPS.contains(&toks[j].lower.as_str()));
                fix(
                    lemma.to_string(),
                    format!("Après « {} », on met l'infinitif : « {lemma} ». Astuce : remplace par « vendre ».", toks[j].text),
                    "verbe",
                    sure,
                );
                continue;
            }
        }

        // 4 bis. juste après le sujet, un infinitif ou un participe : « vous mangé » -> « vous mangez »
        if let Some((p, k)) = subj {
            if verb_first
                && lemma.ends_with("er")
                && !e.is_finite()
                && (e.is_infinitive() || e.is_participle())
                && aux_before(lex, toks, i).is_none()
                && wants_infinitive(lex, toks, i).is_none()
            {
                let cands: Vec<&Entry> = lex.homophones(e).into_iter().filter(|h| h.lemma() == e.lemma()).collect();
                let has = |h: &Entry, tense: u8| h.forms().any(|(_, f)| f.mode == IND && f.tense == tense && f.person == p);
                let target = cands.iter().find(|h| has(h, PRE)).or_else(|| cands.iter().find(|h| has(h, IMPF)));
                if let Some(h) = target {
                    let who = toks[k].lower.trim_end_matches('\'');
                    let who = if who == "j" { "je" } else { who };
                    fix(h.word.clone(), format!("Après « {who} », le verbe est conjugué : « {who} {} ».", h.word), "verbe", true);
                    continue;
                }
            }
        }

        // 5. accord sujet-verbe : « ils mangent », « les chats dorment », « les enfants qui jouent »
        let group_subj = if subj.is_none() && e.is_finite() && (verb_first || !e.has_tag("ADJ")) {
            noun_subj.or_else(|| qui_subject(lex, toks, i))
        } else {
            None
        };
        if let Some((p, k)) = subj.or(group_subj) {
            if e.is_finite() && !e.has_person(p) && (verb_first || i == k + 1 || group_subj.is_some()) {
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
                    let tip = if group_subj.is_some() {
                        let who = pronoun(p);
                        format!(
                            "« {} » = « {who} » : {}",
                            group_label(lex, toks, k),
                            ending_tip(p, who).trim_start_matches(&format!("Avec « {who} », ")[..])
                        )
                    } else {
                        let who = toks[k].lower.trim_end_matches('\'');
                        let who = if who == "j" { "je" } else { who };
                        ending_tip(p, if who.is_empty() { pronoun(p) } else { who })
                    };
                    let sure = match group_subj {
                        None => true,
                        Some((p, k)) => {
                            matches!(toks[k].lower.as_str(), "moi" | "toi" | "lui" | "elle" | "nous" | "vous" | "eux" | "elles")
                                || lex.lookup(&toks[k].lower).is_some_and(|n| {
                                    if p == 6 { n.gn & PLUR != 0 } else { n.gn & SING != 0 }
                                })
                        }
                    };
                    fix(target.to_string(), tip, "accord", sure);
                    continue;
                }
            }
        }

        // 5 bis. juste après un sujet, un mot qui n'est pas un verbe : son homophone verbe
        if let Some((p, k)) = subj {
            if e.verbs.is_empty() && lower.chars().count() > 2 && matches!(e.top_tag(), "NOM" | "ADJ") {
                let verb = lex
                    .homophones(e)
                    .into_iter()
                    .find(|h| h.has_person(p) && matches!(h.top_tag(), "VER" | "AUX"));
                if let Some(h) = verb {
                    let who = toks[k].lower.trim_end_matches('\'');
                    let who = if who == "j" { "je" } else { who };
                    fix(
                        h.word.clone(),
                        format!("Après « {who} », il faut un verbe conjugué : « {} ».", h.word),
                        "homophone",
                        true,
                    );
                    continue;
                }
            }
        }

        // 6. accord avec le déterminant : « les chats », « un cheval », « nos devoirs »
        let noun_like = matches!(e.top_tag(), "NOM" | "ADJ")
            || (e.has_tag("NOM") && prev.is_some_and(|p| det_plur(p) && p != "les"));
        if noun_like && e.gn != 0 && !INVARIABLE.contains(&lower) {
            if let Some((n, j)) = det_number(toks, i, lex) {
                let det = toks[j].text.as_str();
                if n & SING == 0 && e.gn & PLUR == 0 {
                    if let Some(p) = plural_of(lex, lower) {
                        fix(p.clone(), format!("Après « {det} », on met le pluriel : « {p} »."), "accord", true);
                        continue;
                    }
                } else if n & PLUR == 0 && e.gn & SING == 0 {
                    if let Some(s) = singular_of(lex, lower) {
                        fix(s.clone(), format!("Après « {det} », on met le singulier : « {s} »."), "accord", true);
                        continue;
                    }
                }
            }
        }

        // 6 bis. l'adjectif s'accorde avec le nom : « une grande maison », « des chats noirs »
        if let Some((need, noun)) = adj_agreement(lex, toks, i, e) {
            if let Some(f) = agree(lex, lower, need) {
                fix(f.clone(), format!("« {f} » s'accorde avec « {noun} » ({}).", gn_name(need)), "accord", true);
                continue;
            }
        }

        // 7. accord après être : « elle est partie », « les filles sont parties », « la porte est fermée »
        if (e.top_tag() == "ADJ" || e.is_participle()) && e.gn != 0 {
            if let Some(("être", j)) = aux_before(lex, toks, i) {
                // « ils se sont lavé les mains » : le complément vient après, pas d'accord avec le sujet
                let reflexive = (clause_start(toks, j)..j).any(|k| matches!(toks[k].lower.as_str(), "se" | "s'" | "me" | "m'" | "te" | "t'"))
                    || (j >= 2 && matches!(toks[j - 1].lower.as_str(), "nous" | "vous") && matches!(toks[j - 2].lower.as_str(), "nous" | "vous"));
                let det_after = next.is_some_and(|n| det_gn(&n.lower).is_some());
                if let Some((need, who)) = subject_gn(lex, toks, j).filter(|_| !(reflexive && det_after)) {
                    if e.gn & need == 0 {
                        if let Some(f) = agree(lex, lower, need) {
                            fix(f.clone(), format!("Avec « {who} » et le verbe être, on accorde au {} : « {f} ».", gn_name(need)), "accord", true);
                            continue;
                        }
                    }
                }
            }
        }

        // 8. avec avoir, le participe ne s'accorde pas avec le sujet : « elle a mangé »
        if e.is_participle() && verb_first && e.gn != 0 && e.gn & MS == 0 {
            if let Some(("avoir", j)) = aux_before(lex, toks, i) {
                let sent_start = toks[..j].iter().rposition(|x| x.sent != t.sent).map_or(0, |x| x + 1);
                // « les pommes qu'il a mangées », « il les a mangés » : le complément est avant, on accorde
                let subject = subject_before(toks, j).or_else(|| noun_subject(lex, toks, j)).map(|(_, k)| k);
                let pronoun_object = subject.is_some_and(|k| {
                    toks[k + 1..j].iter().any(|x| {
                        matches!(x.lower.as_str(), "l'" | "le" | "la" | "les" | "me" | "m'" | "te" | "t'" | "nous" | "vous")
                    })
                });
                let object_before = pronoun_object
                    || toks[sent_start..j].iter().any(|x| {
                        matches!(
                            x.lower.as_str(),
                            "que" | "qu'" | "combien" | "quel" | "quelle" | "quels" | "quelles" | "lesquels" | "lesquelles" | "laquelle"
                        )
                    });
                if !object_before {
                    if let Some(m) = masculine_participle(lex, lower) {
                        fix(m.clone(), format!("Avec « avoir », le participe ne s'accorde pas avec le sujet : « {m} »."), "accord", true);
                        continue;
                    }
                }
            }
        }
    }
    // les accents rétablis au début restent, sauf si une règle a donné mieux pour le même mot
    for c in pre {
        if !out.iter().any(|o| o.start == c.start) {
            out.push(c);
        }
    }
    out.sort_by_key(|c| c.start);
    out
}

/// « les zamis » : le son de liaison a été écrit au début du mot. Renvoie le mot sans lui.
fn liaison<'a>(lex: &Lexicon, prev: Option<&str>, word: &'a str) -> Option<&'a str> {
    const Z: &[&str] = &[
        "les", "des", "ces", "mes", "tes", "ses", "nos", "vos", "leurs", "aux", "deux", "trois", "six", "dix",
        "nous", "vous", "ils", "elles", "chez", "très", "quelques", "plusieurs", "mes", "dans", "sans",
    ];
    const N: &[&str] = &["un", "on", "en", "mon", "ton", "son", "aucun", "bien", "rien"];
    const T: &[&str] = &["petit", "grand", "quand", "est", "sont", "ont", "fait", "tout"];
    let prev = prev?;
    let rest = word.strip_prefix('z').filter(|_| Z.contains(&prev))
        .or_else(|| word.strip_prefix('n').filter(|_| N.contains(&prev)))
        .or_else(|| word.strip_prefix('t').filter(|_| T.contains(&prev)))?;
    (rest.chars().count() >= 2 && lex.lookup(word).is_none()).then_some(rest)
}

/// Le mot accordé (féminin, pluriel) qui existe dans le dictionnaire.
fn agree(lex: &Lexicon, w: &str, need: u8) -> Option<String> {
    let mut c: Vec<String> = Vec::new();
    if need & MP != 0 {
        if let Some(stem) = w.strip_suffix("al") {
            c.push(format!("{stem}aux"));
        }
        if w.ends_with("eau") {
            c.push(format!("{w}x"));
        }
        if !w.ends_with(['s', 'x']) {
            c.push(format!("{w}s"));
        }
    }
    if need & (MS | MP) == 0 {
        // le féminin : tous les modèles possibles, le dictionnaire dit lequel existe
        let mut fems: Vec<String> = [
            ("eux", "euse"), ("eur", "euse"), ("teur", "trice"), ("if", "ive"), ("f", "ve"), ("el", "elle"), ("eil", "eille"),
            ("il", "ille"), ("en", "enne"), ("on", "onne"), ("et", "ette"), ("et", "ète"), ("er", "ère"), ("c", "che"),
            ("g", "gue"), ("eau", "elle"), ("x", "se"), ("ou", "olle"), ("ul", "ulle"),
        ]
        .iter()
        .filter_map(|(a, b)| w.strip_suffix(a).map(|stem| format!("{stem}{b}")))
        .collect();
        fems.push(format!("{w}e"));
        for fem in fems {
            if need & FS != 0 {
                c.push(fem.clone());
            }
            if need & FP != 0 {
                c.push(format!("{fem}s"));
            }
        }
    }
    c.into_iter().find(|x| lex.lookup(x).is_some_and(|e| e.gn & need != 0))
}

/// Jusqu'où on revient en arrière pour corriger un mot que la suite de la phrase éclaire.
const LOOK_BACK: usize = 60;

fn glued(sentence: &[char], start: usize) -> bool {
    start > 0 && !matches!(sentence[start - 1], ' ' | '\u{a0}' | '(' | '«' | '"' | '\'' | '’' | '-')
}

/// Les corrections à appliquer toutes seules quand un mot se termine (juste avant `term`) :
/// celle du mot lui-même, et celles des mots précédents que ce mot vient confirmer
/// (« il commence a manger » : le « a » devient « à » quand « manger » est tapé).
/// Triées dans l'ordre de la phrase, sans chevauchement. `mode` : voir `custom`.
pub fn auto_fixes(lex: &Lexicon, sentence: &[char], term: char, mode: u8) -> Vec<Correction> {
    if mode == custom::OFF {
        return Vec::new();
    }
    let end = sentence.len();
    let word_len = sentence.iter().rev().take_while(|c| c.is_alphabetic()).count();
    if word_len == 0 || word_len > 40 {
        return Vec::new();
    }
    // le mot qui se termine est collé à autre chose (adresse, fichier, mot de passe) : on ne fait rien
    if glued(sentence, end - word_len) {
        return Vec::new();
    }
    let mut chars = sentence.to_vec();
    chars.push(term);
    let mut out: Vec<Correction> = check(lex, &chars)
        .into_iter()
        .filter(|c| c.auto && end - c.start <= LOOK_BACK)
        .filter(|c| mode == custom::FULL || c.kind == "orthographe")
        .filter(|c| !glued(sentence, c.start))
        // sigles et mots en majuscules (SMS, USB) : jamais
        .filter(|c| !(c.original.chars().count() > 1 && c.original.chars().all(|ch| !ch.is_lowercase())))
        .collect();
    out.sort_by_key(|c| c.start);
    out.dedup_by(|b, a| b.start < a.end);
    out
}

/// La correction du seul mot qui vient de se terminer (pour les tests).
#[cfg(test)]
pub fn auto_fix(lex: &Lexicon, sentence: &[char], term: char, mode: u8) -> Option<Correction> {
    auto_fixes(lex, sentence, term, mode).into_iter().find(|c| c.end == sentence.len())
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
