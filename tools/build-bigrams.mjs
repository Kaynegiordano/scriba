// Modèle « mots qui vont ensemble » (bigrammes) pour Scriba, tiré des phrases françaises de Tatoeba.
// Usage : node tools/build-bigrams.mjs <fra_sentences.tsv> <lexique.tsv> <sortie.tsv> [phrases-test.txt]
//   fra_sentences.tsv : https://downloads.tatoeba.org/exports/per_language/fra/fra_sentences.tsv.bz2 (décompressé)
//   lexique.tsv : l'index de Scriba (src-tauri/data/lexique.tsv) : seuls ses mots sont gardés.
// Sortie : une ligne par mot gardé, du plus fréquent au moins fréquent (le numéro de ligne, à partir de 0,
// est le numéro du mot ; la ligne 0 est « <s> », le début de proposition) :
//   mot \t nombre d'occurrences \t suivants
// suivants : « numéro:compte » séparés par des espaces, numéros croissants (paires vues au moins MIN fois).
// Les mots sont découpés comme dans Scriba (« l' », « qu' » à part ; « aujourd'hui », « peut-être » entiers
// s'ils sont dans le dictionnaire) ; une ponctuation coupe la suite de mots. Sortie reproductible :
// même fichier d'entrée, même sortie (tri complet, sans dépendre de l'ordre de lecture).
// Une phrase sur dix (numéro Tatoeba multiple de 10) n'est pas apprise : on y prend les phrases du banc
// d'essai « fautes en phrase » (phrases-test.txt), pour mesurer sur des phrases que le modèle n'a jamais vues.
import { readFileSync, writeFileSync } from "node:fs";

const [src, lexPath, out, testOut] = process.argv.slice(2);
if (!src || !lexPath || !out) {
  console.error("usage: node build-bigrams.mjs <fra_sentences.tsv> <lexique.tsv> <sortie.tsv>");
  process.exit(1);
}
const MIN = 2; // une paire vue une seule fois n'est souvent qu'un hasard
const MIN_WORD = 2;

const lex = new Set();
for (const line of readFileSync(lexPath, "utf8").split(/\r?\n/)) {
  const w = line.split("\t")[0];
  if (w) lex.add(w); // Lexique écrit « soeur » : on ramène « sœur » à « soeur » (norm)
}
const norm = (w) => w.replace(/œ/g, "oe").replace(/æ/g, "ae");

// Découpe d'une phrase en suites de mots (une suite par proposition, coupée par la ponctuation).
function spans(text) {
  const res = [];
  let cur = [];
  const re = /(\p{L}+'?)|([-])|([^\s\p{L}'’-])|['’]/gu;
  const s = text.toLowerCase().replace(/’/g, "'");
  let m;
  let lastEnd = -1;
  let lastHyphen = false;
  while ((m = re.exec(s))) {
    if (m[1]) {
      let w = m[1];
      const prev = cur[cur.length - 1];
      // recoller « aujourd'hui », « peut-être », comme Scriba
      if (prev && lastEnd === m.index && prev.endsWith("'") && lex.has(norm(prev + w))) {
        cur[cur.length - 1] = prev + w;
      } else if (prev && lastHyphen && lex.has(norm(prev + "-" + w))) {
        cur[cur.length - 1] = prev + "-" + w;
      } else {
        cur.push(w);
      }
      lastEnd = m.index + m[0].length;
      lastHyphen = false;
    } else if (m[2]) {
      lastHyphen = lastEnd === m.index;
    } else if (m[3]) {
      if (cur.length) res.push(cur);
      cur = [];
      lastHyphen = false;
    }
  }
  if (cur.length) res.push(cur);
  return res;
}

const uni = new Map();
const bi = new Map();
const lines = readFileSync(src, "utf8").split(/\r?\n/);
const held = [];
for (const line of lines) {
  const [num, , text] = line.split("\t");
  if (!text) continue;
  if (Number(num) % 10 === 0) {
    held.push([Number(num), text]);
    continue;
  }
  for (const span of spans(text)) {
    let prev = "<s>";
    uni.set(prev, (uni.get(prev) || 0) + 1);
    for (const raw of span) {
      const w = norm(raw);
      // mot inconnu du dictionnaire (nom propre, faute) : il coupe la suite
      if (!lex.has(w)) {
        prev = null;
        continue;
      }
      uni.set(w, (uni.get(w) || 0) + 1);
      if (prev) {
        const k = prev + "\t" + w;
        bi.set(k, (bi.get(k) || 0) + 1);
      }
      prev = w;
    }
  }
}

const cmp = (a, b) => (a < b ? -1 : a > b ? 1 : 0);
const words = [...uni.entries()]
  .filter(([w, c]) => w === "<s>" || c >= MIN_WORD)
  .sort((a, b) => (a[0] === "<s>" ? -1 : b[0] === "<s>" ? 1 : b[1] - a[1] || cmp(a[0], b[0])));
const id = new Map(words.map(([w], i) => [w, i]));
const next = words.map(() => []);
for (const [k, c] of bi) {
  if (c < MIN) continue;
  const [a, b] = k.split("\t");
  if (!id.has(a) || !id.has(b)) continue;
  next[id.get(a)].push([id.get(b), c]);
}
let pairs = 0;
const outLines = words.map(([w, c], i) => {
  const n = next[i].sort((x, y) => x[0] - y[0]);
  pairs += n.length;
  return `${w}\t${c}\t${n.map(([j, k]) => `${j}:${k}`).join(" ")}`;
});
writeFileSync(out, outLines.join("\n") + "\n");
console.log(`${words.length} mots, ${pairs} paires -> ${out}`);

if (testOut) {
  // phrases courtes et simples, tous les mots connus, sans nom propre ni chiffre
  const TEST = 3000;
  const ok = (t) => {
    if (!/^[A-ZÀ-Ÿa-zà-ÿ][^0-9"«»()\[\]…;:]*[.!?]$/u.test(t) || t.includes("  ")) return false;
    const words = t.split(/[\s,.!?'’-]+/).filter(Boolean);
    if (words.length < 4 || words.length > 12) return false;
    if (words.slice(1).some((w) => /\p{Lu}/u.test(w))) return false;
    return spans(t).flat().every((w) => lex.has(norm(w)));
  };
  const picked = held.sort((a, b) => a[0] - b[0]).filter(([, t]) => ok(t.replace(/’/g, "'"))).slice(0, TEST);
  writeFileSync(testOut, picked.map(([, t]) => t.replace(/’/g, "'")).join("\n") + "\n");
  console.log(`${picked.length} phrases de test (sur ${held.length} mises de côté) -> ${testOut}`);
}
