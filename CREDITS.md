# Crédits

Scriba est un outil à usage privé. Il s'appuie sur ces ressources libres :

- **Lexique 3.83** — Boris New, Christophe Pallier et coll., [lexique.org](http://www.lexique.org). Licence CC BY-SA 4.0.
  Mots, prononciations, fréquences, analyses des verbes et syllabes (`src-tauri/data/lexique.tsv`).
- **Verbiste** — Pierre Sarrazin, [sarrazip.com/dev/verbiste.html](http://sarrazip.com/dev/verbiste.html). Licence GPL.
  Modèles de conjugaison (`src-tauri/data/conj.tsv`).
- **Tatoeba** — phrases françaises des contributeurs de [tatoeba.org](https://tatoeba.org). Licence CC BY 2.0 FR.
  Modèle « mots qui vont ensemble » (combien de fois un mot en suit un autre, `src-tauri/data/bigrams.tsv`)
  et phrases du banc d'essai (`src-tauri/data/phrases-test.txt`, phrases mises de côté, non apprises).
  Source : export `fra_sentences.tsv.bz2` du 19 octobre 2026
  (https://downloads.tatoeba.org/exports/per_language/fra/fra_sentences.tsv.bz2,
  sha256 `b49a8777f4be14ba765eb591b7f4136b684e9d5a88a3b028a0ebcf172e1f6326`) ; l'export change chaque semaine :
  garder ce fichier pour reproduire exactement les mêmes données.
- **Pictogrammes ARASAAC** — auteur : Sergio Palao. Origine : ARASAAC ([arasaac.org](https://arasaac.org)).
  Propriété : Gouvernement d'Aragon (Espagne). Licence CC BY-NC-SA (usage non commercial).
  Correspondance mots / pictogrammes et images des mots courants (`src-tauri/data/pictos.tsv`, `pictos.bin`).
- **OpenDyslexic** — Abbie Gonzalez, [opendyslexic.org](https://opendyslexic.org). SIL Open Font License 1.1 (`src/fonts/OFL.txt`).

Regénérer les données : `npm run index`, `npm run conj`, `npm run bigrams` (après `npm run index`), puis
`node tools/build-pictos.mjs …` et `python -I tools/fetch-pictos.py …` (voir l'en-tête des scripts).

## Grammalecte

Correction grammaticale de toute la phrase : Grammalecte 2.3.0 (https://grammalecte.net), licence GPL 3. Il n'est pas inclus dans Scriba.exe : Scriba le télécharge au premier lancement depuis grammalecte.net (empreinte SHA-256 vérifiée) et le fait tourner avec Python.
