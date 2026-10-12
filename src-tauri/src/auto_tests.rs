//! Tests de la correction automatique et des règles ajoutées avec elle.

use crate::custom;
use crate::grammar::{auto_fix, auto_fixes, check};
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
        ("les chats dorme", ' ', "dorme", "dorment"),
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
        ("il commence a", ' '), // a / à dépend du mot suivant : rien tant qu'il n'est pas tapé
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

/// Toutes les corrections faites quand le dernier mot se termine (y compris sur les mots d'avant).
fn all(s: &str, term: char) -> Vec<(String, String)> {
    let chars: Vec<char> = s.chars().collect();
    auto_fixes(lex(), &chars, term, custom::FULL).into_iter().map(|c| (c.original, c.replacement)).collect()
}

#[test]
fn selon_la_phrase() {
    let yes = [
        // le mot d'avant, corrigé quand la suite le confirme
        ("il commence a manger", "a", "à"),
        ("il et grand", "et", "est"),
        ("il et très grand", "et", "est"),
        ("sa va", "sa", "ça"),
        ("il ma dit", "ma", "m'a"),
        ("elle la mangé", "la", "l'a"),
        ("se matin", "se", "ce"),
        ("leur parents", "leur", "leurs"),
        // le mot lui-même, choisi selon ce qui précède
        ("il ces", "ces", "sait"),
        ("je ces", "ces", "sais"),
        ("on cent", "cent", "sent"),
        ("les mezon", "mezon", "maisons"),
        ("il a manjé", "manjé", "mangé"),
        ("il va manjé", "manjé", "manger"),
        ("ils manjai", "manjai", "mangeaient"),
        ("les zami", "zami", "amis"),
        ("des zenfan", "zenfan", "enfants"),
        ("un nami", "nami", "ami"),
    ];
    let mut failed = Vec::new();
    for (s, from, to) in yes {
        let got = all(s, ' ');
        println!("{s:<24} -> {got:?}");
        if !got.iter().any(|(a, b)| a == from && b == to) {
            failed.push(s);
        }
    }
    let clean = [
        "il commence à manger",
        "elle et moi",
        "sa porte",
        "il sait",
        "les maisons",
        "il la mange",
        "ma mère",
        "il se lave",
        "leur dit",
        "elle aussi",
        "il est grand",
        "je sais",
        "nous deux",
        "on sent",
    ];
    let noisy: Vec<String> = clean
        .iter()
        .filter_map(|s| {
            let f = all(s, ' ');
            (!f.is_empty()).then(|| format!("{s} -> {f:?}"))
        })
        .collect();
    assert!(failed.is_empty() && noisy.is_empty(), "manqués : {failed:?}\nfausses alertes : {noisy:#?}");
}

#[test]
fn syllabes() {
    let l = lex();
    assert_eq!(l.syllables("maison"), vec!["mai", "son"]);
    assert_eq!(l.syllables("Chocolat"), vec!["Cho", "co", "lat"]);
    assert_eq!(l.syllables("youtube"), vec!["youtube"]);
}

#[test]
fn phrase_longue() {
    for s in ["ça va et il commence a manger", "il commence a manger", "et il commence a manger"] {
        let got = all(s, ',');
        println!("{s:<34} + ',' -> {got:?}");
        assert!(got.iter().any(|(a, b)| a == "a" && b == "à"), "{s}");
    }
}

#[test]
fn mots_rates_corriges_seuls() {
    // mal écrits : la meilleure proposition est appliquée sans F7
    let yes = [
        ("on va a l'ospital", ' ', "ospital", "hôpital"),
        ("il fait un exersice", ' ', "exersice", "exercice"),
        ("mon professur", ' ', "professur", "professeur"),
        ("un livre intéressan", ' ', "intéressan", "intéressant"),
        ("c'est un probleme", ' ', "probleme", "problème"),
        ("nous avons un devoire", ' ', "devoire", "devoir"),
        ("la matematique", ' ', "matematique", "mathématique"),
    ];
    for (s, term, from, to) in yes {
        let got = auto(s, term, custom::FULL);
        println!("{s:<26} + {term:?} -> {got:?}");
        assert_eq!(got, Some((from.to_string(), to.to_string())), "{s}");
    }
    // corrects ou ambigus : on ne touche à rien
    for s in ["Paul mange", "il y a Amélie", "on a rendez-vous", "la maison est grande", "mon prénom est Kylian"] {
        for term in [' ', '.'] {
            let got = fixes(&format!("{s}{term}")).into_iter().find(|_| false);
            println!("{s:<26} -> {got:?}");
            let auto = auto(s, term, custom::FULL);
            assert_eq!(auto, None, "{s}");
        }
    }
}

#[test]
fn etonnants() {
    for (s, term) in [
        ("j'ai un iphone", ' '), ("mon iphone", ' '), ("l'iphone", ' '), ("je ne", ' '), ("je ne sais", ' '),
        ("je n'ai", ' '), ("je ne veux", ' '), ("je ne suis", ' '), ("je ne", '.'), ("ne", ' '), ("je nes", ' '),
        ("youtube", ' '), ("sur whatsapp", ' '), ("sur tiktok", ' '), ("il joue a fortnite", ' '), ("mon pc", ' '),
    ] {
        println!("{s:<24} + {term:?} -> {:?}", auto(s, term, custom::FULL));
    }
}

#[test]
fn mots_a_laisser() {
    let mut changed = Vec::new();
    for w in [
        "mdr", "ptdr", "wesh", "genre", "ouf", "meuf", "kiffer", "chelou", "tkt", "frero", "gamer", "gaming", "skin", "skins",
        "lol", "stream", "like", "liker", "follow", "story", "reels", "tiktok", "snap", "insta", "gta", "fifa", "mbappe",
        "messi", "ronaldo", "zelda", "mario", "sonic", "pixel", "pc", "usb", "hdmi", "ram", "ssd", "gpu", "wifi", "ps5",
        "chatgpt", "gpt", "claude", "ia", "emoji", "twitch", "paypal", "amazon", "uber", "tesla", "lego", "naruto", "goku",
        "okay", "bref", "daron", "reuf", "relou", "zinzin", "wallah", "bg", "oklm", "jpp", "svp", "stp", "bcp", "pk", "pcq",
    ] {
        for sentence in [format!("c'est {w}"), format!("j'ai {w}"), format!("mon {w}")] {
            if let Some((a, b)) = auto(&sentence, ' ', custom::FULL) {
                if !changed.contains(&format!("{a}>{b}")) {
                    changed.push(format!("{a}>{b}"));
                }
            }
        }
    }
    println!("CHANGES: {changed:?}");
}
