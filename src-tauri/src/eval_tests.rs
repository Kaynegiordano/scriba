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
    ("il y en a olein d'autres ", "il y en a plein d'autres "),
    ("il y en a plien ", "il y en a plein "),
    ("il mange une pome ", "il mange une pomme "),
    ("elle est come ça ", "elle est comme ça "),
    ("ça ces trop bien", "ça c'est trop bien"),
    ("Paul corrgie le texte", "Paul corrige le texte"),
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
    ("les enfants ont mangé des pates a la sauce ", "les enfants ont mangé des pâtes à la sauce "),
    ("mon frère mange des pates a la sauce ", "mon frère mange des pâtes à la sauce "),
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
    // sujets coordonnés, accents tapés, e muet
    ("le chat et le chien joue.", "le chat et le chien jouent."),
    ("le garçon et la fille sont gentil.", "le garçon et la fille sont gentils."),
    ("la fille et sa mère sont partie.", "la fille et sa mère sont parties."),
    ("les filles sont gentil.", "les filles sont gentilles."),
    ("une bonne idé ", "une bonne idée "),
    ("le telefone sonne.", "le téléphone sonne."),
    ("il sai nager.", "il sait nager."),
    ("elles se sont lavé ensemble.", "elles se sont lavées ensemble."),
    ("Mezon de campagne ", "Maison de campagne "),
];

/// Phrases justes : rien ne doit bouger.
const JUSTES: &[&str] = &[
    "ils ont fait ça ces mecs",
    "donne-moi ça ces jouets sont à moi",
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
    // prénoms en tête de phrase, coordinations
    "Léa mange une pomme. ",
    "Hugo et Paul jouent. ",
    "Marc a faim. ",
    "Il mange une pomme et le chien aboie. ",
    "Le chat mange et le chien dort. ",
    "Les chats et les chiens jouent. ",
    "Ils se sont lavés. ",
    "Elle la fait. ",
    "Il la dit. ",
    "Tout est prêt. ",
    "Il a tout mangé. ",
    "Tout le monde est là. ",
    "Il pleut et en plus il fait froid. ",
    "Ce que tu dis est vrai. ",
    "De ce fait, il part. ",
    "Ce fut un succès. ",
    "Mon père et ma mère ont dit non. ",
    "Ma mère a dit non. ",
    "Elle ne la regarde pas. ",
    "Ce qui compte, c'est toi. ",
    "Nous avons fini. ",
    "Il a fini ses devoirs. ",
    "Elle a pris le bus. ",
    "Le chien et le chat se battent. ",
    "Ils se battent tout le temps. ",
    "Tous les jours, il lit. ",
    "Toutes les filles sont parties. ",
    "Il est parti tôt. ",
    "Les professeurs nous ont donné des devoirs. ",
    "Mes parents sont contents. ",
    "Je m'en vais. ",
    "Elle s'en va. ",
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
    // le banc est complet : toute régression fait échouer les tests
    assert!(ko.is_empty() && abimes.is_empty(), "{} fautes ratées, {} phrases justes abîmées", ko.len(), abimes.len());
}

#[test]
fn debug_eval() {
    // SCRIBA_DEBUG="phrase 1|phrase 2" pour examiner d'autres phrases
    let env = std::env::var("SCRIBA_DEBUG").unwrap_or_else(|_| "je mangeait |on mangent |je veu |tu mangeait ".into());
    for s in env.split('|') {
        let chars: Vec<char> = s.chars().collect();
        println!("{s:?} check -> {:?}", crate::grammar::check(lex(), &chars).iter().map(|c| (c.original.clone(), c.replacement.clone(), c.auto, c.reason.clone())).collect::<Vec<_>>());
        println!("   tape -> {:?}", tape(s));
        let chars: Vec<char> = s.trim_end().chars().collect();
        println!("   auto -> {:?}", auto_fixes(lex(), &chars, ' ', custom::FULL).iter().map(|c| (c.original.clone(), c.replacement.clone())).collect::<Vec<_>>());
    }
}


/// Fautes fabriquées sur les mots les plus courants : combien sont rattrapées, combien mal corrigées.
/// `cargo test --release corpus -- --nocapture`
#[test]
fn corpus() {
    let words = lex().top_words(4000);
    let mut seed: u64 = 12345;
    let mut rnd = |n: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % n.max(1)
    };
    let strip = |w: &str| crate::lexicon::fold(w);
    type Tr = (&'static str, fn(&str, usize) -> Option<String>);
    let transforms: Vec<Tr> = vec![
        ("sans accent", |w, _| { let f = crate::lexicon::fold(w); (f != w).then_some(f) }),
        ("lettre oubliée", |w, r| { let c: Vec<char> = w.chars().collect(); let i = 1 + r % (c.len() - 1); let mut o = c.clone(); o.remove(i); Some(o.into_iter().collect()) }),
        ("lettre en trop", |w, r| { let c: Vec<char> = w.chars().collect(); let i = r % c.len(); let mut o = c.clone(); o.insert(i, c[i]); Some(o.into_iter().collect()) }),
        ("lettres inversées", |w, r| { let c: Vec<char> = w.chars().collect(); if c.len() < 4 { return None; } let i = 1 + r % (c.len() - 2); let mut o = c.clone(); o.swap(i, i + 1); Some(o.into_iter().collect()) }),
        ("double consonne simplifiée", |w, _| { let c: Vec<char> = w.chars().collect(); let i = (1..c.len()).find(|&i| c[i] == c[i - 1] && !"aeiouy".contains(c[i]))?; let mut o = c.clone(); o.remove(i); Some(o.into_iter().collect()) }),
        ("consonne doublée à tort", |w, _| { let c: Vec<char> = w.chars().collect(); let i = (1..c.len() - 1).find(|&i| !"aeiouyàâéèêîôûh".contains(c[i]) && c[i] != c[i - 1] && c[i] != c[i + 1])?; let mut o = c.clone(); o.insert(i, c[i]); Some(o.into_iter().collect()) }),
        ("lettre voisine (clavier)", |w, r| { let c: Vec<char> = w.chars().collect(); let i = 1 + r % (c.len() - 1); let near = match c[i] { 'a' => 'z', 'z' => 'e', 'e' => 'r', 'r' => 't', 't' => 'y', 'i' => 'o', 'o' => 'p', 'q' => 's', 's' => 'd', 'd' => 'f', 'n' => 'b', 'm' => 'l', 'u' => 'i', 'l' => 'k', 'c' => 'v', _ => return None }; let mut o = c.clone(); o[i] = near; Some(o.into_iter().collect()) }),
        ("phonétique", |w, _| { let f = crate::lexicon::fold(w); let r = f.replace("ph", "f").replace("qu", "k").replace("ch", "sh").replace("ain", "in").replace("eau", "o").replace("ou", "u").replace("tion", "sion").replace("ç", "s").replace("gu", "g").replace("oi", "oa"); (r != f).then_some(r) }),
        ("fin muette oubliée", |w, _| { let c: Vec<char> = w.chars().collect(); (c.len() >= 5 && matches!(c[c.len() - 1], 's' | 't' | 'e' | 'x' | 'd')).then(|| c[..c.len() - 1].iter().collect()) }),
    ];
    let mut total_fixed = 0;
    let mut total_wrong = 0;
    let mut total = 0;
    for (name, f) in &transforms {
        let (mut good, mut wrong, mut miss, mut n) = (0, 0, 0, 0);
        let mut wrongs: Vec<String> = Vec::new();
        let mut misses: Vec<String> = Vec::new();
        for w in &words {
            let Some(m) = f(w, rnd(1000)) else { continue };
            if m == *w || lex().lookup(&m).is_some() || m.chars().count() < 3 { continue; }
            n += 1;
            let got = tape(&format!("voici {m} "));
            let got = got.trim_end().strip_prefix("voici ").unwrap_or("?");
            if got == w.as_str() { good += 1; }
            else if got == m { miss += 1; if misses.len() < 30 { misses.push(format!("{m}→{w}")); } }
            else { wrong += 1; if wrongs.len() < 40 { wrongs.push(format!("{m}→{got} (voulu {w})")); } }
        }
        let _ = strip;
        println!("\n{name:<28} n={n:<5} bien={good:<5} ratés={miss:<5} mauvais={wrong}");
        println!("   ratés   : {}", misses.join(" · "));
        println!("   mauvais : {}", wrongs.join(" · "));
        total_fixed += good; total_wrong += wrong; total += n;
    }
    println!("\nTOTAL corrigés {total_fixed}/{total} ({:.0} %), mal corrigés {total_wrong} ({:.1} %)", 100.0 * total_fixed as f64 / total as f64, 100.0 * total_wrong as f64 / total as f64);
}

#[test]
fn debug_words() {
    use crate::lexicon::{Hint, PLUR};
    for (w, n) in [("mezon", None), ("mezon", Some(PLUR))] {
        let h = Hint { number: n, ..Hint::default() };
        println!("{w} {n:?} confident={:?} suggest={:?}", lex().confident(w, &h), lex().suggest(w, &h, 6).iter().map(|s| s.word.clone()).collect::<Vec<_>>());
    }
}

#[test]
fn timing() {
    let worst = "xqzt blorf wesh kiffer gamer tiktok fortnite roblox minecraft zorbax plouf mdr ptdr gnagna frzzt blabla yolo ouaip nikel ";
    let chars: Vec<char> = worst.chars().collect();
    let t = std::time::Instant::now();
    for _ in 0..20 {
        let _ = auto_fixes(lex(), &chars, ' ', custom::FULL);
    }
    println!("pire phrase : {:?} par mot terminé", t.elapsed() / 20);
    let t = std::time::Instant::now();
    for _ in 0..20 {
        let _ = crate::grammar::check(lex(), &chars);
    }
    println!("check complet : {:?}", t.elapsed() / 20);
}

/// Fautes d'élève variées, non utilisées pour régler les règles : mesure de généralisation (non bloquant).
#[test]
fn nouvelles_phrases() {
    let cas: &[(&str, &str)] = &[
        ("hier je suis aller au cinema avec mes amis ", "hier je suis allé au cinéma avec mes amis "),
        ("mon frere et ma soeur sont partie en vacance ", "mon frère et ma sœur sont partis en vacances "),
        ("il fait tres beau aujourdhui ", "il fait très beau aujourd'hui "),
        ("jai pas compri la lesson ", "j'ai pas compris la leçon "),
        ("les professeur nous on donner beaucoup de devoir ", "les professeurs nous ont donné beaucoup de devoirs "),
        ("je voudrait un nouveau telephone ", "je voudrais un nouveau téléphone "),
        ("elle a perdu ses clef hier soir ", "elle a perdu ses clés hier soir "),
        ("on ses vu au parc samedi ", "on s'est vu au parc samedi "),
        ("tu peut venir si tu veut ", "tu peux venir si tu veux "),
        ("cest la maison de mon ami ", "c'est la maison de mon ami "),
        ("il y a beaucoup de voiture dans la rue ", "il y a beaucoup de voitures dans la rue "),
        ("nous avons manger des pomme ", "nous avons mangé des pommes "),
        ("les enfant joue dans le jardin ", "les enfants jouent dans le jardin "),
        ("la fille est tombé dans lescalier ", "la fille est tombée dans l'escalier "),
        ("je suis très content de te voir ", "je suis très content de te voir "),
        ("il est partie sans dire au revoir ", "il est parti sans dire au revoir "),
        ("elle sappelle sophie et elle a douze an ", "elle s'appelle sophie et elle a douze ans "),
        ("mes parent son contant ", "mes parents sont contents "),
        ("on va a la plage demain ", "on va à la plage demain "),
        ("il faut que tu fasse tes devoirs ", "il faut que tu fasses tes devoirs "),
        ("ma mere ma dit de ranger ma chambre ", "ma mère m'a dit de ranger ma chambre "),
        ("je ne sais pas ce que je vais faire ", "je ne sais pas ce que je vais faire "),
        ("les fleur sont tres belle ", "les fleurs sont très belles "),
        ("nous sommes aller au restaurent ", "nous sommes allés au restaurant "),
        ("il a oublier son cartable a la maison ", "il a oublié son cartable à la maison "),
        ("mon chien et mon chat ce battent tout le temps ", "mon chien et mon chat se battent tout le temps "),
        ("je pence que cest une bonne idée ", "je pense que c'est une bonne idée "),
        ("elle ma donner un cadeau ", "elle m'a donné un cadeau "),
        ("ils sont arrivé en retard ", "ils sont arrivés en retard "),
        ("la voiture de mon père et en panne ", "la voiture de mon père est en panne "),
        ("tout les jours je vais a lecole ", "tous les jours je vais à l'école "),
        ("il pleu depuis ce matin ", "il pleut depuis ce matin "),
        ("je men vais ", "je m'en vais "),
        ("se matin jai oublier mon sac ", "ce matin j'ai oublié mon sac "),
        ("les élèves on fini leur exercice ", "les élèves ont fini leur exercice "),
        ("elle est tres jolie dans sa robe rouge ", "elle est très jolie dans sa robe rouge "),
        ("nous allons mangez des crepes ", "nous allons manger des crêpes "),
        ("jaime beaucoup le chocolat ", "j'aime beaucoup le chocolat "),
        ("il ne ma pas répondu ", "il ne m'a pas répondu "),
        ("ces amis son gentil ", "ses amis sont gentils "),
        ("je vais chez mon ami pour jouer au foot ", "je vais chez mon ami pour jouer au foot "),
        ("quand il est arrivé jetais deja parti ", "quand il est arrivé j'étais déjà parti "),
        ("la maitresse a expliquer la leçon ", "la maîtresse a expliqué la leçon "),
        ("jai mal a la tete ", "j'ai mal à la tête "),
        ("les gens son fatigué ", "les gens sont fatigués "),
        ("mon pére travail beaucoup ", "mon père travaille beaucoup "),
        ("il mange du poison pane ", "il mange du poisson pané "),
        ("la porte est ouvert ", "la porte est ouverte "),
        ("nous avont finit ", "nous avons fini "),
        ("tu as bien travailler ", "tu as bien travaillé "),
        ("je les ai vu hier ", "je les ai vus hier "),
    ];
    let mut ok = 0;
    for (tape_, voulu) in cas {
        let got = tape(tape_);
        if got == *voulu { ok += 1 } else { println!("  RATÉ  {tape_:<52} -> {got:<52} (voulu : {voulu})"); }
    }
    println!("NOUVELLES : {ok}/{}", cas.len());
    assert!(ok >= 38, "généralisation en recul : {ok}/{}", cas.len());
}

#[test]
fn debug_sent() {
    let Ok(list) = std::env::var("SENT") else { return };
    for s in list.split('|') {
        let chars: Vec<char> = format!("{s} ").chars().collect();
        println!("SENT {s:?}");
        for c in crate::grammar::check(lex(), &chars) {
            println!("    {} -> {} (auto={}) {}", c.original, c.replacement, c.auto, c.reason);
        }
        println!("    tape: {:?}", tape(&format!("{s} ")));
    }
}
