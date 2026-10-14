//! Banc d'essai : phrases d'élève tapées lettre par lettre, comme dans une vraie application.
//! `cargo test --release eval -- --nocapture` affiche le score et les écarts.

use crate::custom;
use crate::grammar::auto_fixes;
use crate::lexicon::tests::lex;

/// Frappe réelle : à chaque espace ou ponctuation, la correction automatique passe
/// (comme le crochet clavier), puis la touche est tapée.
pub fn tape(phrase: &str) -> String {
    let mut typed: Vec<char> = Vec::new();
    for c in phrase.chars() {
        let term = matches!(c, ' ' | ',' | '.' | ';' | ':' | '!' | '?' | ')');
        if term && typed.last().is_some_and(|x| x.is_alphabetic()) && (c == ' ' || typed.contains(&' ')) {
            for f in auto_fixes(lex(), &typed, c, custom::FULL).into_iter().rev() {
                typed.splice(f.start..f.end, f.replacement.chars());
            }
        }
        typed.push(c);
    }
    typed.into_iter().collect()
}

/// (tapé, attendu)
const FAUTES: &[(&str, &str)] = &[
    // accords dans le groupe du nom
    ("une grand maison ", "une grande maison "),
    ("la petit fille ", "la petite fille "),
    ("des chats noir ", "des chats noirs "),
    ("une robe vert ", "une robe verte "),
    ("les enfants malade ", "les enfants malades "),
    ("deux chat ", "deux chats "),
    ("trois livre ", "trois livres "),
    ("le maison ", "la maison "),
    ("un maison ", "une maison "),
    ("une chien ", "un chien "),
    ("mon maison ", "ma maison "),
    ("ma chien ", "mon chien "),
    ("ce maison ", "cette maison "),
    ("ce arbre ", "cet arbre "),
    ("cette chien ", "ce chien "),
    ("ça maison ", "sa maison "),
    // accords avec être (sujet groupe du nom)
    ("la porte est fermé.", "la porte est fermée."),
    ("les fruits sont mur.", "les fruits sont mûrs."),
    ("les filles sont parti.", "les filles sont parties."),
    ("ma soeur est content.", "ma sœur est contente."),
    ("nous sommes content.", "nous sommes contents."),
    // participe avec avoir : pas d'accord avec le sujet
    ("elle a mangée une pomme ", "elle a mangé une pomme "),
    ("elles ont chantées ", "elles ont chanté "),
    // -é / -er / -ez
    ("le chat a manger ", "le chat a mangé "),
    ("mon frère a jouer ", "mon frère a joué "),
    ("vous mangé ", "vous mangez "),
    ("vous parler trop ", "vous parlez trop "),
    ("il faut mangé ", "il faut manger "),
    ("je vais joué ", "je vais jouer "),
    ("avant de mangé ", "avant de manger "),
    // accord sujet-verbe
    ("les enfants qui joue ", "les enfants qui jouent "),
    ("je mangeait ", "je mangeais "),
    ("tu mangeait ", "tu mangeais "),
    ("ils mangeait ", "ils mangeaient "),
    ("on mangent ", "on mange "),
    ("les élèves écoute ", "les élèves écoutent "),
    // homophones
    ("les enfants on mangé ", "les enfants ont mangé "),
    ("les enfants son partis ", "les enfants sont partis "),
    ("ont va au parc ", "on va au parc "),
    ("le chat à mangé ", "le chat a mangé "),
    ("il mange des pates a la sauce ", "il mange des pâtes à la sauce "),
    ("je joue a la console ", "je joue à la console "),
    ("ces un chat ", "c'est un chat "),
    ("ces pour toi ", "c'est pour toi "),
    ("ces quoi ", "c'est quoi "),
    ("mes il est parti ", "mais il est parti "),
    ("il est la.", "il est là."),
    ("le ciel et bleu ", "le ciel est bleu "),
    ("quand il et parti ", "quand il est parti "),
    ("il a mis sont manteau ", "il a mis son manteau "),
    // mots collés, apostrophes
    ("quil vienne ", "qu'il vienne "),
    ("cest bien ", "c'est bien "),
    ("parceque ", "parce que "),
    ("daccord ", "d'accord "),
    ("dabord ", "d'abord "),
    ("aujourdhui ", "aujourd'hui "),
    ("quelquun ", "quelqu'un "),
    ("il sest levé ", "il s'est levé "),
    ("je mappelle ", "je m'appelle "),
    ("tu tes trompé ", "tu t'es trompé "),
    ("sava ", "ça va "),
    ("ilya ", "il y a "),
    ("biensur ", "bien sûr "),
    ("tanpis ", "tant pis "),
    ("peutetre ", "peut-être "),
    // mots écrits comme ils se prononcent
    ("je sui aller a la piscine ", "je suis allé à la piscine "),
    ("mon anniverssaire ", "mon anniversaire "),
    ("la bibliotek ", "la bibliothèque "),
    ("il fait tro chaud ", "il fait trop chaud "),
    ("toujour ", "toujours "),
    ("le cinema ", "le cinéma "),
    ("je regarde la télé avec mon pere ", "je regarde la télé avec mon père "),
    ("les vacanse ", "les vacances "),
    ("il pleu ", "il pleut "),
    ("j'aprend ", "j'apprends "),
    ("le telefone ", "le téléphone "),
    ("je veu ", "je veux "),
    ("il etait une foi ", "il était une fois "),
    ("on sai jamais ", "on sait jamais "),
    ("il fesait beau ", "il faisait beau "),
    ("j'ai pri le bus ", "j'ai pris le bus "),
    ("on a fini nos devoir ", "on a fini nos devoirs "),
    ("une foi ", "une fois "),
    ("ses amis sont gentil.", "ses amis sont gentils."),
];

/// Phrases justes : rien ne doit bouger.
const JUSTES: &[&str] = &[
    "Le chat de ma grand-mère a la patte cassée. ",
    "Paul a faim, il mange une pomme. ",
    "Ma mère et mon père sont partis au travail. ",
    "Les pommes qu'il a mangées étaient bonnes. ",
    "Il les a mangés hier soir. ",
    "Elle l'a mangée. ",
    "La tarte à la fraise est délicieuse. ",
    "Le chat à côté de moi dort. ",
    "Je vais à l'école à pied. ",
    "Ces chats sont mignons. ",
    "Ses parents sont gentils. ",
    "C'est un bon livre. ",
    "Il s'est levé tôt. ",
    "Mais il est parti. ",
    "Il est là. ",
    "Il est là-bas. ",
    "Passe par la porte. ",
    "Le ciel et la mer sont bleus. ",
    "On a mangé des pâtes. ",
    "Ils ont mangé. ",
    "Ont-ils fini ? ",
    "Son frère est grand. ",
    "Ils sont grands. ",
    "Vous parlez trop. ",
    "Vous êtes gentil. ",
    "Je mangeais une glace. ",
    "Il mangeait. ",
    "Les enfants qui jouent sont contents. ",
    "Le livre que j'ai lu. ",
    "Il faut manger. ",
    "Elle porte une robe verte. ",
    "Il le porte. ",
    "Elle la porte. ",
    "Un des enfants est malade. ",
    "Mon amie est là. ",
    "Cet arbre est grand. ",
    "Cette maison est belle. ",
    "Ce matin il pleut. ",
    "Sa maison est grande. ",
    "Ça va bien. ",
    "La porte est fermée. ",
    "Nous sommes contents. ",
    "Il y a deux chats. ",
    "Les élèves écoutent le professeur. ",
    "Je joue à la console avec mes amis. ",
    "On va au parc. ",
    "Il a mis son manteau. ",
    "Quand il est parti, j'ai pleuré. ",
    "Le petit chat noir dort. ",
    "La grande maison blanche. ",
    "Des chats noirs. ",
    "Les enfants malades restent chez eux. ",
    "Tout à coup il a crié. ",
    "Je suis allé à la piscine. ",
    "Il y a une tache sur la chemise. ",
    "J'ai lu un livre sur les dinosaures. ",
    "Il a lu la lettre. ",
    "Il l'a lue. ",
    "Le professeur nous a donné des devoirs. ",
    "Elle a une sœur. ",
    "Elles ont chanté. ",
    "Je vous parle. ",
    "Il fait chaud, on va à la plage. ",
    "Tu as fait tes devoirs ? ",
    "Quelqu'un a sonné. ",
    "Mes amis et moi, on joue au foot. ",
    "Le chien a mangé sa pâtée. ",
    "Ma sœur est contente. ",
    "Les filles sont parties. ",
    "Mon frère a joué. ",
    "La maison de mon oncle est à côté. ",
    "Je pense à toi. ",
    "C'est à moi. ",
    "À demain ! ",
    "Il a peur du noir. ",
    "Le garçon a une belle voix. ",
    "Où est mon sac ? ",
    "Tu veux du thé ou du café ? ",
    "Je le vois. ",
    "Le match est fini. ",
    "La fin du film est triste. ",
    "Mon père a un vélo. ",
    "Mon prof de maths est sympa. ",
    "La boulangerie est fermée le lundi. ",
    "Ils se sont lavé les mains. ",
    "Elle s'est lavée. ",
    "Il est midi. ",
    "Elle est médecin. ",
    "Les vacances sont finies. ",
    "Le chat et le chien jouent. ",
    "Elle et moi. ",
    "Ce sont mes amis. ",
    "Je me suis trompé. ",
    "Tu t'es trompé. ",
    "Il a fait ses devoirs. ",
    "Le film est nul. ",
    "J'ai vu un film trop bien. ",
    "On s'est bien amusés. ",
    "Le roi est mort. ",
    "La reine a un château. ",
    "Le père de Léa est pompier. ",
    "Ma mère m'a dit non. ",
    "La pluie tombe. ",
    "Ce que je veux. ",
    "Le sport que je préfère, c'est le foot. ",
    "Elle a mal à la tête. ",
    "Il pleut à verse. ",
    "Le temps passe vite. ",
    "Le bus a du retard. ",
    "Les chiens aboient. ",
    "Le chien aboie. ",
    "Les feuilles tombent. ",
    "La classe est vide. ",
    "Mon cousin habite à Paris. ",
    "Nous avons gagné le match. ",
    "Vous avez raison. ",
    "Il était une fois un roi. ",
    "La nuit tombe vite en hiver. ",
    "Ma tante a deux chiens. ",
    "Leur maison est jolie. ",
    "Je leur parle. ",
    "Le musée est ouvert. ",
    "Les portes sont ouvertes. ",
    "La pizza est prête. ",
    "Elles sont prêtes. ",
    "Ils sont prêts. ",
    "J'ai faim. ",
    "Le gâteau a brûlé. ",
    "La voiture de mon père est rouge. ",
    "Des fleurs rouges. ",
    "Une fleur rouge. ",
    "Un grand merci. ",
    "Une grande fête. ",
];

#[test]
fn eval() {
    let mut ok = 0;
    let mut ko = Vec::new();
    for (tape_, voulu) in FAUTES {
        let got = tape(tape_);
        if got == *voulu {
            ok += 1;
        } else {
            ko.push(format!("{tape_:<38} -> {got:<38} (voulu : {voulu})"));
        }
    }
    let mut abimes = Vec::new();
    for s in JUSTES {
        let got = tape(s);
        if got != *s {
            abimes.push(format!("{s:<45} -> {got}"));
        }
        // et en minuscules, sans majuscule de début de phrase
        let low = {
            let mut c = s.chars();
            c.next().map(|f| f.to_lowercase().chain(c).collect::<String>()).unwrap_or_default()
        };
        let got = tape(&low);
        if got != low {
            abimes.push(format!("{low:<45} -> {got}"));
        }
    }
    println!("\nFAUTES corrigées : {ok}/{}", FAUTES.len());
    for k in &ko {
        println!("  RATÉ  {k}");
    }
    println!("JUSTES abîmées : {}/{}", abimes.len(), JUSTES.len() * 2);
    for a in &abimes {
        println!("  ABÎMÉ {a}");
    }
    let strict = std::env::var("SCRIBA_EVAL_STRICT").is_ok();
    if strict {
        assert!(abimes.is_empty());
    }
}

#[test]
fn debug_eval() {
    for s in ["je mangeait ", "on mangent ", "je veu ", "tu mangeait "] {
        let chars: Vec<char> = s.chars().collect();
        println!("{s:?} check -> {:?}", crate::grammar::check(lex(), &chars).iter().map(|c| (c.original.clone(), c.replacement.clone(), c.auto)).collect::<Vec<_>>());
        let chars: Vec<char> = s.trim_end().chars().collect();
        println!("   auto -> {:?}", auto_fixes(lex(), &chars, ' ', custom::FULL).iter().map(|c| (c.original.clone(), c.replacement.clone())).collect::<Vec<_>>());
    }
}
