# Scriba

Aide à l'écriture en français pour Windows, pensée pour un élève de collège (4e) qui écrit avec des difficultés d'orthographe.
Une petite barre flotte en haut de l'écran et suit ce qui est tapé **dans n'importe quelle application**.

- **Propositions de mots** : on écrit « comme on entend » (`mezon`, `fonetik`), Scriba propose `maison`, `phonétique`. Touches F1 à F6 ou clic. Pictogrammes ARASAAC à côté des mots.
- **Correction automatique** au fil de la frappe, selon le sens de la phrase : accents, accords (`les chat`, `une grand maison`), verbes (`il a manger`, `ils mange`), homophones (`a / à`, `et / est`, `ces / c'est`, `ont / on`…), mots collés (`parceque`), apostrophes oubliées (`quil`). Retour arrière juste après = annuler, et Scriba n'y revient plus.
- **Conjugaison** (Ctrl+Alt+C) : tous les temps simples et composés, formes pronominales, terminaisons en couleur.
- **Lecture** (Ctrl+Alt+R) : texte sélectionné lu à voix haute, syllabes colorées, police OpenDyslexic, règle de lecture.
- **Calculatrice** (Ctrl+Alt+K) : le résultat est aussi écrit en lettres et lu à voix haute.
- Léger (aucun serveur, tout est dans l'exécutable de 14 Mo), rien n'est envoyé sur Internet : les mots tapés restent sur le PC.

Raccourcis : Ctrl+Alt+L affiche ou masque la barre, F7 applique la correction proposée, F8 ajoute un mot au dictionnaire.
Le menu de l'icône de la zone de notification règle la correction, les pictogrammes, la lecture audio et le démarrage avec Windows.

## Installation et mises à jour

Télécharger `Scriba.exe` dans la [dernière publication](https://github.com/Kaynegiordano/scriba/releases/latest) (exécutable portable, WebView2 requis, présent sous Windows 11).

Scriba cherche lui-même les nouvelles versions 30 secondes après le démarrage puis toutes les 6 heures. Quand une version est disponible, la barre affiche un bouton : un clic télécharge, vérifie l'empreinte SHA-256, remplace l'exécutable et redémarre. Aussi dans le menu : « Rechercher une mise à jour… ». `Scriba.exe --update` met à jour sans interface (code de sortie 0 = installée, 10 = déjà à jour, 1 = échec).

## Développement

```
npm install
npm run index      # dictionnaire (Lexique 3.83) -> src-tauri/data/lexique.tsv
npm run conj       # modèles de conjugaison (Verbiste) -> src-tauri/data/conj.tsv
npm run bigrams    # mots qui vont ensemble (Tatoeba) -> src-tauri/data/bigrams.tsv, phrases-test.txt
npx tauri build --no-bundle    # -> src-tauri/target/release/scriba.exe
cd src-tauri && cargo test --release    # moteur, banc d'essai de phrases
```

Le banc d'essai `eval_tests.rs` tape de vraies phrases lettre par lettre : toutes les fautes prévues doivent être corrigées et aucune phrase juste ne doit être abîmée.

Pour publier : monter la version dans `package.json`, `src-tauri/Cargo.toml` et `src-tauri/tauri.conf.json`, construire, puis `gh release create vX.Y.Z src-tauri/target/release/scriba.exe#Scriba.exe`. Le fichier doit s'appeler `Scriba.exe`.

## Licences et crédits

Voir [CREDITS.md](CREDITS.md) : Lexique 3.83 (CC BY-SA 4.0), Verbiste (GPL), phrases Tatoeba (CC BY 2.0 FR), pictogrammes ARASAAC (CC BY-NC-SA, usage non commercial), OpenDyslexic (OFL). Scriba est un outil à usage privé et non commercial.
