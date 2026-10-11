// Convertit Lexique383.tsv en index compact pour Scriba.
// Usage : node tools/build-index.mjs <Lexique383.tsv> <sortie.tsv>
// Sortie (une ligne par mot) : mot \t phons(|) \t fréquence \t catégories(,) \t lemme verbe \t infover
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";

const [src, out] = process.argv.slice(2);
if (!src || !out) {
  console.error("usage: node build-index.mjs <Lexique383.tsv> <sortie.tsv>");
  process.exit(1);
}

const lines = readFileSync(src, "utf8").split(/\r?\n/);
const head = lines[0].split("\t");
const col = (n) => head.indexOf(n);
const C = {
  ortho: col("ortho"), phon: col("phon"), lemme: col("lemme"), cgram: col("cgram"),
  ff: col("freqfilms2"), fl: col("freqlivres"), infover: col("infover"),
};

const words = new Map();
const phonChars = new Set();
for (let i = 1; i < lines.length; i++) {
  const f = lines[i].split("\t");
  if (f.length < head.length - 5) continue;
  const ortho = f[C.ortho];
  const phon = f[C.phon];
  if (!ortho || !phon || ortho.includes(" ")) continue;
  for (const ch of phon) phonChars.add(ch);
  let w = words.get(ortho);
  if (!w) {
    w = { phons: new Set(), freq: 0, tags: new Map(), lemma: "", infover: new Set() };
    words.set(ortho, w);
  }
  w.phons.add(phon);
  const fr = ((parseFloat(f[C.ff]) || 0) + (parseFloat(f[C.fl]) || 0)) / 2;
  w.freq += fr;
  const cg = f[C.cgram];
  if (cg) w.tags.set(cg, (w.tags.get(cg) || 0) + fr);
  if ((cg === "VER" || cg === "AUX") && f[C.infover]) {
    if (!w.lemma || cg === "VER") w.lemma = f[C.lemme];
    for (const iv of f[C.infover].split(";")) if (iv) w.infover.add(iv);
  }
}

const rows = [];
for (const [ortho, w] of words) {
  rows.push([
    ortho, [...w.phons].join("|"), w.freq.toFixed(2),
    [...w.tags].sort((a, b) => b[1] - a[1]).map(([t]) => t).join(","),
    w.lemma, [...w.infover].join(";"),
  ].join("\t"));
}
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, rows.join("\n") + "\n", "utf8");
console.log(`${rows.length} mots -> ${out}`);
console.log("phonèmes :", [...phonChars].sort().join(" "));
