//! Tableaux de conjugaison (données Verbiste, GPL — Pierre Sarrazin) :
//! temps simples, temps composés avec le bon auxiliaire, élision (j'aime).

use crate::lexicon::fold;
use serde::Serialize;
use std::collections::HashMap;

// position des cases dans un modèle (voir tools/build-conj.mjs)
const INF: usize = 0;
const IND_PRE: usize = 1;
const IND_IMP: usize = 7;
const IND_FUT: usize = 13;
const IND_PAS: usize = 19;
const CND_PRE: usize = 25;
const SUB_PRE: usize = 31;
const IMPER: usize = 43;
const PAR_PRE: usize = 46;
const PAR_PAS: usize = 47;

/// Verbes conjugués avec « être » aux temps composés.
const WITH_ETRE: &[&str] = &[
    "aller", "arriver", "partir", "repartir", "venir", "revenir", "devenir", "redevenir", "parvenir", "survenir",
    "intervenir", "advenir", "provenir", "entrer", "rentrer", "sortir", "ressortir", "naître", "renaître",
    "mourir", "rester", "tomber", "retomber", "monter", "remonter", "descendre", "redescendre", "passer",
    "retourner", "apparaître", "décéder", "échoir",
];

struct Verb {
    template: usize,
    aspirate: bool,
}

pub struct Conjugator {
    templates: Vec<(String, Vec<Vec<String>>)>,
    verbs: HashMap<String, Verb>,
    sorted: Vec<(String, String)>,
}

#[derive(Serialize)]
pub struct Row {
    /// 0..5 = je..ils (impératif : 0..2 = tu, nous, vous)
    pub person: usize,
    pub pronoun: String,
    /// auxiliaire des temps composés (« ai », « suis »)
    pub aux: String,
    pub form: String,
}

#[derive(Serialize)]
pub struct Tense {
    pub mode: &'static str,
    pub name: &'static str,
    pub tip: &'static str,
    pub rows: Vec<Row>,
}

#[derive(Serialize)]
pub struct Table {
    pub verb: String,
    pub group: &'static str,
    pub aux: &'static str,
    pub present_participle: String,
    pub past_participle: String,
    pub tenses: Vec<Tense>,
}

fn starts_with_vowel(s: &str) -> bool {
    s.chars().next().is_some_and(|c| "aeiouyàâäéèêëîïôöûùüœh".contains(c))
}

impl Conjugator {
    pub fn load(data: &str) -> Self {
        let mut templates = Vec::new();
        let mut verbs = HashMap::new();
        for line in data.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f.as_slice() {
                ["T", name, cells] => {
                    let cells = cells.split('|').map(|c| c.split('/').filter(|v| !v.is_empty()).map(String::from).collect()).collect();
                    templates.push((name.to_string(), cells));
                }
                ["V", verb, t, asp] => {
                    if let Ok(t) = t.parse() {
                        verbs.insert(verb.to_string(), Verb { template: t, aspirate: *asp == "1" });
                    }
                }
                _ => {}
            }
        }
        let mut sorted: Vec<(String, String)> = verbs.keys().map(|v| (fold(v), v.clone())).collect();
        sorted.sort();
        Conjugator { templates, verbs, sorted }
    }

    pub fn knows(&self, verb: &str) -> bool {
        self.verbs.contains_key(verb)
    }

    /// Verbes qui commencent par `prefix` (accents ignorés).
    pub fn search(&self, prefix: &str, max: usize) -> Vec<String> {
        let p = fold(prefix);
        let lo = self.sorted.partition_point(|(k, _)| k.as_str() < p.as_str());
        self.sorted[lo..].iter().take_while(|(k, _)| k.starts_with(&p)).take(max).map(|(_, v)| v.clone()).collect()
    }

    /// Les formes d'une case (première variante) : radical du verbe + terminaison du modèle.
    fn cell(&self, verb: &str, idx: usize) -> Option<String> {
        let v = self.verbs.get(verb)?;
        let (name, cells) = &self.templates[v.template];
        let suffix = name.split_once(':').map(|(_, s)| s).unwrap_or(name);
        let radical = verb.strip_suffix(suffix).unwrap_or(verb);
        cells.get(idx)?.first().map(|end| format!("{radical}{end}"))
    }

    pub fn conjugate(&self, verb: &str) -> Option<Table> {
        let v = self.verbs.get(verb)?;
        let etre = WITH_ETRE.contains(&verb);
        let aux = if etre { "être" } else { "avoir" };
        let elide = |form: &str| starts_with_vowel(form) && !(form.starts_with('h') && v.aspirate);
        let pp = self.cell(verb, PAR_PAS).unwrap_or_default();

        let pronouns = ["je", "tu", "il / elle / on", "nous", "vous", "ils / elles"];
        let subject = |p: usize, first: &str, que: bool| -> String {
            let base = if p == 0 && elide(first) { "j'".to_string() } else { pronouns[p].to_string() };
            if !que {
                return base;
            }
            if p == 2 || p == 5 { format!("qu'{base}") } else { format!("que {base}") }
        };
        let joined = |pronoun: &str| if pronoun.ends_with('\'') { String::new() } else { " ".into() };

        let simple = |mode: &'static str, name: &'static str, tip: &'static str, start: usize, que: bool| -> Tense {
            let rows = (0..6)
                .filter_map(|p| {
                    let form = self.cell(verb, start + p)?;
                    let pronoun = subject(p, &form, que);
                    Some(Row { person: p, pronoun: format!("{pronoun}{}", joined(&pronoun)), aux: String::new(), form })
                })
                .collect();
            Tense { mode, name, tip, rows }
        };
        let compound = |name: &'static str, tip: &'static str, aux_start: usize| -> Tense {
            let rows = (0..6)
                .filter_map(|p| {
                    let a = self.cell(aux, aux_start + p)?;
                    let form = match (etre, p) {
                        (true, 0..=2) => format!("{pp}(e)"),
                        (true, _) => format!("{pp}(e)s"),
                        (false, _) => pp.clone(),
                    };
                    if pp.is_empty() {
                        return None;
                    }
                    let pronoun = subject(p, &a, false);
                    Some(Row { person: p, pronoun: format!("{pronoun}{}", joined(&pronoun)), aux: format!("{a} "), form })
                })
                .collect();
            Tense { mode: "Indicatif", name, tip, rows }
        };

        let mut tenses = vec![
            simple("Indicatif", "Présent", "", IND_PRE, false),
            simple("Indicatif", "Imparfait", "-ais, -ais, -ait, -ions, -iez, -aient pour tous les verbes", IND_IMP, false),
            simple("Indicatif", "Futur simple", "infinitif + -ai, -as, -a, -ons, -ez, -ont", IND_FUT, false),
            simple("Indicatif", "Passé simple", "", IND_PAS, false),
            compound("Passé composé", "auxiliaire au présent + participe passé", IND_PRE),
            compound("Plus-que-parfait", "auxiliaire à l'imparfait + participe passé", IND_IMP),
            compound("Futur antérieur", "auxiliaire au futur + participe passé", IND_FUT),
            simple("Conditionnel", "Présent", "radical du futur + terminaisons de l'imparfait", CND_PRE, false),
            simple("Subjonctif", "Présent", "que je -e, -es, -e, -ions, -iez, -ent", SUB_PRE, true),
        ];
        let imper: Vec<Row> = [(0usize, "(tu)"), (1, "(nous)"), (2, "(vous)")]
            .iter()
            .filter_map(|(k, who)| {
                Some(Row { person: *k, pronoun: format!("{who} "), aux: String::new(), form: self.cell(verb, IMPER + k)? })
            })
            .collect();
        tenses.push(Tense { mode: "Impératif", name: "Présent", tip: "", rows: imper });
        tenses.retain(|t| !t.rows.is_empty());

        let group = if verb.ends_with("er") && verb != "aller" {
            "1er groupe"
        } else if verb.ends_with("ir") && self.cell(verb, IND_PRE + 3).is_some_and(|f| f.ends_with("issons")) {
            "2e groupe"
        } else {
            "3e groupe"
        };
        Some(Table {
            verb: self.cell(verb, INF).unwrap_or_else(|| verb.to_string()),
            group,
            aux,
            present_participle: self.cell(verb, PAR_PRE).unwrap_or_default(),
            past_participle: pp,
            tenses,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn c() -> &'static Conjugator {
        static C: OnceLock<Conjugator> = OnceLock::new();
        C.get_or_init(|| Conjugator::load(include_str!("../data/conj.tsv")))
    }

    fn forms(verb: &str, tense: &str, mode: &str) -> Vec<String> {
        let t = c().conjugate(verb).unwrap();
        let t = t.tenses.iter().find(|x| x.name == tense && x.mode == mode).unwrap();
        t.rows.iter().map(|r| format!("{}{}{}", r.pronoun, r.aux, r.form)).collect()
    }

    #[test]
    fn tableaux() {
        assert_eq!(forms("manger", "Présent", "Indicatif")[3], "nous mangeons");
        assert_eq!(forms("manger", "Passé simple", "Indicatif")[4], "vous mangeâtes");
        assert_eq!(forms("aimer", "Présent", "Indicatif")[0], "j'aime");
        assert_eq!(forms("haïr", "Présent", "Indicatif")[0], "je hais");
        assert_eq!(forms("habiter", "Présent", "Indicatif")[0], "j'habite");
        assert_eq!(forms("aller", "Passé composé", "Indicatif")[0], "je suis allé(e)");
        assert_eq!(forms("aller", "Passé composé", "Indicatif")[5], "ils / elles sont allé(e)s");
        assert_eq!(forms("prendre", "Passé composé", "Indicatif")[0], "j'ai pris");
        assert_eq!(forms("pleuvoir", "Présent", "Indicatif")[0], "il / elle / on pleut");
    }

    #[test]
    fn subjonctif_et_groupes() {
        assert_eq!(forms("finir", "Présent", "Subjonctif")[2], "qu'il / elle / on finisse");
        assert_eq!(forms("avoir", "Présent", "Subjonctif")[0], "que j'aie");
        assert_eq!(c().conjugate("finir").unwrap().group, "2e groupe");
        assert_eq!(c().conjugate("courir").unwrap().group, "3e groupe");
        assert_eq!(c().conjugate("aller").unwrap().group, "3e groupe");
        assert!(c().search("mang", 5).contains(&"manger".to_string()));
    }
}
