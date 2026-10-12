//! Tests de la correction automatique et des règles ajoutées avec elle.

use crate::custom;
use crate::grammar::{auto_fix, check};
use crate::lexicon::tests::lex;

fn auto(s: &str, term: char, mode: u8) -> Option<(String, String)> {
    let chars: Vec<char> = s.chars().collect();
    auto_fix(lex(), &chars, term, mode).map(|c| (c.original, c.replacement))
}

fn fixes(s: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = s.chars().collect();
    check(lex(), &chars).into_iter().map(|c| (c.original, c.replacement)).collect()
}

#[test]
fn applique_seul() {
    let yes = [
        ("la mezon", ' ', "mezon", "maison"),
        ("une ecole", ' ', "ecole", "école"),
        ("j'ai fait bocou", ' ', "bocou", "beaucoup"),
        ("c'est dificil", '.', "dificil", "difficile"),
        ("Bonojur", ' ', "Bonojur", "Bonjour"),
        ("elle a emfant", ' ', "emfant", "enfant"),
        ("ils mange", ' ', "mange", "mangent"),
        ("il a manger", ' ', "manger", "mangé"),
        ("elle est parti", ' ', "parti", "partie"),
        ("tu peu", ' ', "peu", "peux"),
        ("ils son", ' ', "son", "sont"),
        ("les chat", ',', "chat", "chats"),
    ];
    let mut failed = Vec::new();
    for (s, term, from, to) in yes {
        let got = auto(s, term, custom::FULL);
        println!("{s:<22} + {term:?} -> {got:?}");
        if got != Some((from.to_string(), to.to_string())) {
            failed.push(s);
        }
    }
    assert!(failed.is_empty(), "non appliquées : {failed:?}");
}

#[test]
fn ne_touche_pas() {
    let no = [
        ("il nous parle", ' '),
        ("je parle avec Paul", ' '),
        ("sur youtube", ' '),
        ("envoie un SMS", ' '),
        ("mon pseudo", ' '),
        ("ke", ' '),
        ("les chats dorme", ' '), // sujet nom : proposé (F7), pas appliqué seul
        ("il commence a", ' '),   // a / à dépend du mot suivant
        ("jean@gmail", '.'),
        ("www.mezon", '.'),
        ("mon_fichier", '.'),
        ("le 3eme", ' '),
        ("il et", ' '),
        ("c'est super", ' '),
    ];
    let hit: Vec<_> = no.iter().filter_map(|(s, t)| auto(s, *t, custom::FULL).map(|r| format!("{s} -> {r:?}"))).collect();
    assert!(hit.is_empty(), "corrigé à tort : {hit:#?}");
}

#[test]
fn modes() {
    assert!(auto("la mezon", ' ', custom::SPELLING).is_some());
    assert!(auto("ils mange", ' ', custom::SPELLING).is_none());
    assert!(auto("la mezon", ' ', custom::OFF).is_none());
    assert!(auto("ils mange", ' ', custom::FULL).is_some());
}

#[test]
fn nouvelles_regles() {
    let cases = [
        ("il ma dit ", "ma", "m'a"),
        ("elle ta donné ", "ta", "t'a"),
        ("il la mangé ", "la", "l'a"),
        ("se chat est gros ", "se", "ce"),
        ("il peu venir ", "peu", "peut"),
        ("elle est parti ", "parti", "partie"),
        ("elles sont content ", "content", "contentes"),
        ("ils sont content ", "content", "contents"),
        ("elle est très grand ", "grand", "grande"),
        ("elle est tombé ", "tombé", "tombée"),
    ];
    let mut failed = Vec::new();
    for (s, from, to) in cases {
        let f = fixes(s);
        println!("{s:<26} -> {f:?}");
        if !f.iter().any(|(a, b)| a == from && b == to) {
            failed.push(s);
        }
    }
    let clean = [
        "elle est partie ",
        "il est parti ",
        "ils sont partis ",
        "elle est grande ",
        "je suis content ",
        "nous sommes contents ",
        "il la mange ",
        "il se lave ",
        "ma mère est là ",
        "la vue est belle ",
        "il mange peu ",
        "elle est ici ",
        "ils sont gentils ",
        "elles sont parties ",
    ];
    let noisy: Vec<String> = clean
        .iter()
        .filter_map(|s| {
            let f = fixes(s);
            (!f.is_empty()).then(|| format!("{s} -> {f:?}"))
        })
        .collect();
    assert!(failed.is_empty() && noisy.is_empty(), "manqués : {failed:?}\nfausses alertes : {noisy:#?}");
}
