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
const SUB_IMP: usize = 37;
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
    /// forme pronominale (« se laver »)
    pub pronominal: bool,
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

    /// « se laver », « s'habiller » -> (« laver », vrai) ; « manger » -> (« manger », faux).
    pub fn split_pronominal(query: &str) -> (&str, bool) {
        let q = query.trim();
        for p in ["se ", "s'", "s\u{2019}"] {
            if let Some(rest) = q.strip_prefix(p) {
                return (rest.trim(), true);
            }
        }
        (q, false)
    }

    pub fn conjugate(&self, verb: &str, pronominal: bool) -> Option<Table> {
        let v = self.verbs.get(verb)?;
        // les verbes pronominaux se conjuguent toujours avec « être » aux temps composés
        let etre = pronominal || WITH_ETRE.contains(&verb);
        let aux = if etre { "être" } else { "avoir" };
        let elide = |form: &str| starts_with_vowel(form) && !(form.starts_with('h') && v.aspirate);
        let pp = self.cell(verb, PAR_PAS).unwrap_or_default();

        let pronouns = ["je", "tu", "il / elle / on", "nous", "vous", "ils / elles"];
        let reflexive = ["me", "te", "se", "nous", "vous", "se"];
        // « je », « tu »… ; pronominal : « je me », « il / elle / on s' », « nous nous »
        let subject = |p: usize, first: &str, que: bool| -> String {
            let mut base = if pronominal {
                let r = reflexive[p];
                if matches!(r, "me" | "te" | "se") && elide(first) {
                    format!("{} {}'", pronouns[p], &r[..1])
                } else {
                    format!("{} {r}", pronouns[p])
                }
            } else if p == 0 && elide(first) {
                "j'".to_string()
            } else {
                pronouns[p].to_string()
            };
            if que {
                base = if p == 2 || p == 5 { format!("qu'{base}") } else { format!("que {base}") };
            }
            base
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
        // temps composé : auxiliaire (au temps donné) + participe passé, accordé au besoin
        let compound = |mode: &'static str, name: &'static str, tip: &'static str, aux_start: usize, que: bool| -> Tense {
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
                    let pronoun = subject(p, &a, que);
                    Some(Row { person: p, pronoun: format!("{pronoun}{}", joined(&pronoun)), aux: format!("{a} "), form })
                })
                .collect();
            Tense { mode, name, tip, rows }
        };

        let mut tenses = vec![
            simple("Indicatif", "Présent", "", IND_PRE, false),
            simple("Indicatif", "Imparfait", "-ais, -ais, -ait, -ions, -iez, -aient pour tous les verbes", IND_IMP, false),
            simple("Indicatif", "Futur simple", "infinitif + -ai, -as, -a, -ons, -ez, -ont", IND_FUT, false),
            simple("Indicatif", "Passé simple", "", IND_PAS, false),
            compound("Indicatif", "Passé composé", "auxiliaire au présent + participe passé", IND_PRE, false),
            compound("Indicatif", "Plus-que-parfait", "auxiliaire à l'imparfait + participe passé", IND_IMP, false),
            compound("Indicatif", "Futur antérieur", "auxiliaire au futur + participe passé", IND_FUT, false),
            compound("Indicatif", "Passé antérieur", "auxiliaire au passé simple + participe passé", IND_PAS, false),
            simple("Conditionnel", "Présent", "radical du futur + terminaisons de l'imparfait", CND_PRE, false),
            compound("Conditionnel", "Passé", "auxiliaire au conditionnel présent + participe passé", CND_PRE, false),
            simple("Subjonctif", "Présent", "que je -e, -es, -e, -ions, -iez, -ent", SUB_PRE, true),
            compound("Subjonctif", "Passé", "auxiliaire au subjonctif présent + participe passé", SUB_PRE, true),
            simple("Subjonctif", "Imparfait", "temps littéraire, rare à l'écrit", SUB_IMP, true),
        ];
        // impératif : « mange », « mangeons » ; pronominal : « lave-toi », « lavons-nous »
        let imper_pronoun = ["toi", "nous", "vous"];
        let imper: Vec<Row> = [(0usize, "(tu)"), (1, "(nous)"), (2, "(vous)")]
            .iter()
            .filter_map(|(k, who)| {
                let form = self.cell(verb, IMPER + k)?;
                let form = if pronominal { format!("{form}-{}", imper_pronoun[*k]) } else { form };
                Some(Row { person: *k, pronoun: format!("{who} "), aux: String::new(), form })
            })
            .collect();
        tenses.push(Tense { mode: "Impératif", name: "Présent", tip: "", rows: imper });
        // impératif passé : « aie mangé », « ayons mangé » (rare, pas de forme pronominale)
        if !pronominal && !pp.is_empty() {
            let past: Vec<Row> = [(0usize, "(tu)"), (1, "(nous)"), (2, "(vous)")]
                .iter()
                .filter_map(|(k, who)| {
                    let a = self.cell(aux, IMPER + k)?;
                    let form = if etre { format!("{pp}(e){}", if *k == 0 { "" } else { "s" }) } else { pp.clone() };
                    Some(Row { person: *k, pronoun: format!("{who} "), aux: format!("{a} "), form })
                })
                .collect();
            tenses.push(Tense { mode: "Impératif", name: "Passé", tip: "impératif de l'auxiliaire + participe passé", rows: past });
        }
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
            pronominal,
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
        let t = c().conjugate(verb, false).unwrap();
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
        assert_eq!(c().conjugate("finir", false).unwrap().group, "2e groupe");
        assert_eq!(c().conjugate("courir", false).unwrap().group, "3e groupe");
        assert_eq!(c().conjugate("aller", false).unwrap().group, "3e groupe");
        assert!(c().search("mang", 5).contains(&"manger".to_string()));
    }

    fn pron(verb: &str, tense: &str, mode: &str) -> Vec<String> {
        let t = c().conjugate(verb, true).unwrap();
        let t = t.tenses.iter().find(|x| x.name == tense && x.mode == mode).unwrap();
        t.rows.iter().map(|r| format!("{}{}{}", r.pronoun, r.aux, r.form)).collect()
    }

    #[test]
    fn temps_composes_et_pronominaux() {
        assert_eq!(forms("manger", "Passé", "Conditionnel")[0], "j'aurais mangé");
        assert_eq!(forms("manger", "Passé", "Subjonctif")[0], "que j'aie mangé");
        assert_eq!(forms("manger", "Passé", "Subjonctif")[2], "qu'il / elle / on ait mangé");
        assert_eq!(forms("manger", "Passé antérieur", "Indicatif")[3], "nous eûmes mangé");
        assert_eq!(forms("manger", "Passé", "Impératif")[1], "(nous) ayons mangé");
        assert_eq!(forms("partir", "Passé", "Conditionnel")[3], "nous serions parti(e)s");
        assert_eq!(forms("manger", "Imparfait", "Subjonctif")[0], "que je mangeasse");
        assert_eq!(pron("laver", "Présent", "Indicatif")[0], "je me lave");
        assert_eq!(pron("laver", "Présent", "Indicatif")[3], "nous nous lavons");
        assert_eq!(pron("habiller", "Présent", "Indicatif")[0], "je m'habille");
        assert_eq!(pron("habiller", "Présent", "Indicatif")[2], "il / elle / on s'habille");
        assert_eq!(pron("laver", "Passé composé", "Indicatif")[2], "il / elle / on s'est lavé(e)");
        assert_eq!(pron("laver", "Présent", "Impératif")[0], "(tu) lave-toi");
        assert_eq!(pron("laver", "Présent", "Subjonctif")[0], "que je me lave");
        assert_eq!(Conjugator::split_pronominal("s'habiller"), ("habiller", true));
        assert_eq!(Conjugator::split_pronominal("se laver"), ("laver", true));
        assert_eq!(Conjugator::split_pronominal("manger"), ("manger", false));
    }
}
