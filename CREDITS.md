# Crédits

Scriba est un outil à usage privé. Il s'appuie sur ces ressources libres :

- **Lexique 3.83** — Boris New, Christophe Pallier et coll., [lexique.org](http://www.lexique.org). Licence CC BY-SA 4.0.
  Mots, prononciations, fréquences, analyses des verbes et syllabes (`src-tauri/data/lexique.tsv`).
- **Verbiste** — Pierre Sarrazin, [sarrazip.com/dev/verbiste.html](http://sarrazip.com/dev/verbiste.html). Licence GPL.
  Modèles de conjugaison (`src-tauri/data/conj.tsv`).
- **Pictogrammes ARASAAC** — auteur : Sergio Palao. Origine : ARASAAC ([arasaac.org](https://arasaac.org)).
  Propriété : Gouvernement d'Aragon (Espagne). Licence CC BY-NC-SA (usage non commercial).
  Correspondance mots / pictogrammes et images des mots courants (`src-tauri/data/pictos.tsv`, `pictos.bin`).
- **OpenDyslexic** — Abbie Gonzalez, [opendyslexic.org](https://opendyslexic.org). SIL Open Font License 1.1 (`src/fonts/OFL.txt`).

Regénérer les données : `npm run index`, `npm run conj`, puis
`node tools/build-pictos.mjs …` et `python -I tools/fetch-pictos.py …` (voir l'en-tête des scripts).
