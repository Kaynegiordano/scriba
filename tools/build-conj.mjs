// Convertit les données Verbiste (GPL, Pierre Sarrazin) en table compacte pour Scriba.
// Usage : node tools/build-conj.mjs <dossier data de verbiste> <sortie.tsv>
// Sortie :
//   T \t nom-du-modèle \t 51 cases séparées par | (variantes séparées par /)
//   V \t verbe \t numéro du modèle \t h aspiré (1/0)
// Ordre des cases : infinitif(1), ind. présent, imparfait, futur, passé simple (6 chacun),
// conditionnel présent(6), subj. présent(6), subj. imparfait(6), impératif(3),
// participe présent(1), participe passé(4 : ms mp fs fp)
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [dir, out] = process.argv.slice(2);
if (!dir || !out) {
  console.error("usage: node build-conj.mjs <verbiste/data> <sortie.tsv>");
  process.exit(1);
}

const conj = readFileSync(join(dir, "conjugation-fr.xml"), "utf8");
const verbs = readFileSync(join(dir, "verbs-fr.xml"), "utf8");

const lines = [];
const index = new Map();
for (const m of conj.matchAll(/<template name="([^"]+)">([\s\S]*?)<\/template>/g)) {
  const [, name, body] = m;
  const cells = [...body.matchAll(/<p>([\s\S]*?)<\/p>|<p\s*\/>/g)].map((p) =>
    [...(p[1] ?? "").matchAll(/<i>([^<]*)<\/i>/g)].map((i) => i[1]).join("/"),
  );
  if (cells.length !== 51) throw new Error(`${name} : ${cells.length} cases au lieu de 51`);
  index.set(name, index.size);
  lines.push(["T", name, cells.join("|")].join("\t"));
}

let n = 0;
for (const m of verbs.matchAll(/<v>\s*<i>([^<]+)<\/i>\s*<t>([^<]+)<\/t>\s*(<aspirate-h\s*\/>)?\s*<\/v>/g)) {
  const [, verb, tpl, asp] = m;
  if (!index.has(tpl)) continue;
  lines.push(["V", verb, index.get(tpl), asp ? 1 : 0].join("\t"));
  n++;
}
writeFileSync(out, lines.join("\n") + "\n", "utf8");
console.log(`${index.size} modèles, ${n} verbes -> ${out}`);
