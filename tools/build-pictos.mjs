// Prépare les pictogrammes ARASAAC (Sergio Palao, Gouvernement d'Aragon, CC BY-NC-SA) pour Scriba.
// Usage : node tools/build-pictos.mjs <all-fr.json> <lexique.tsv> <pictos.tsv> <ids-courants.txt> [nombre]
//   pictos.tsv        : mot \t numéro du pictogramme (mots d'un seul tenant, minuscules)
//   ids-courants.txt  : numéros des pictogrammes des mots les plus courants (images à garder sur le PC)
import { readFileSync, writeFileSync } from "node:fs";

const [src, lexique, out, topOut, countArg] = process.argv.slice(2);
if (!src || !lexique || !out || !topOut) {
  console.error("usage: node build-pictos.mjs <all-fr.json> <lexique.tsv> <pictos.tsv> <ids.txt> [nombre]");
  process.exit(1);
}
const TOP = Number(countArg ?? 1500);

const all = JSON.parse(readFileSync(src, "utf8"));
// mot -> { id, score } : le pictogramme dont c'est le sens principal, de préférence « CAA »
const best = new Map();
let kept = 0;
for (const p of all) {
  if (p.violence || p.sex) continue; // outil pour un collégien
  kept++;
  (p.keywords ?? []).forEach((k, rank) => {
    for (const form of [k.keyword, k.plural]) {
      const w = (form ?? "").trim().toLowerCase();
      if (!w || /\s/.test(w)) continue;
      const score = (rank === 0 ? 4 : 0) + (form === k.keyword ? 2 : 0) + (p.aac ? 1 : 0) - rank * 0.1;
      const cur = best.get(w);
      if (!cur || score > cur.score) best.set(w, { id: p._id, score });
    }
  });
}

const rows = [...best].sort((a, b) => a[0].localeCompare(b[0])).map(([w, v]) => `${w}\t${v.id}`);
writeFileSync(out, rows.join("\n") + "\n", "utf8");

// les mots les plus fréquents (Lexique) qui ont un pictogramme
const freq = new Map();
for (const line of readFileSync(lexique, "utf8").split("\n")) {
  const [w, , f] = line.split("\t");
  if (w) freq.set(w, parseFloat(f) || 0);
}
const ranked = [...best].filter(([w]) => freq.has(w)).sort((a, b) => freq.get(b[0]) - freq.get(a[0]));
const ids = [];
const seen = new Set();
for (const [, v] of ranked) {
  if (ids.length >= TOP) break;
  if (!seen.has(v.id)) {
    seen.add(v.id);
    ids.push(v.id);
  }
}
writeFileSync(topOut, ids.join("\n") + "\n", "utf8");
console.log(`${kept} pictogrammes gardés, ${rows.length} mots -> ${out} ; ${ids.length} images courantes -> ${topOut}`);
