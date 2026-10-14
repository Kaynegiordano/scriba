// Convertit Lexique383.tsv en index compact pour Scriba.
// Usage : node tools/build-index.mjs <Lexique383.tsv> <sortie.tsv>
// Sortie (une ligne par mot) :
//   mot \t phons(|) \t fréquence \t catégories(, par fréquence) \t verbes \t genre-nombre 	 syllabes (mai-son)
// verbes : « lemme=ind:pre:3s;sub:pre:3s|lemme2=… » (lemmes par fréquence)
// genre-nombre : masque hexadécimal 1=ms 2=mp 4=fs 8=fp
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
  genre: col("genre"), nombre: col("nombre"),
  ff: col("freqfilms2"), fl: col("freqlivres"), infover: col("infover"),
  syll: col("orthosyll"),
};

function gnMask(genre, nombre) {
  const gs = genre === "m" ? ["m"] : genre === "f" ? ["f"] : ["m", "f"];
  const ns = nombre === "s" ? ["s"] : nombre === "p" ? ["p"] : ["s", "p"];
  const bit = { ms: 1, mp: 2, fs: 4, fp: 8 };
  let m = 0;
  for (const g of gs) for (const n of ns) m |= bit[g + n];
  return m;
}

// Noms courants dont Lexique ne donne pas le genre (« maison », « voiture »…) : sans lui,
// Scriba ne saurait pas corriger « le maison ». Seuls les noms qui n'ont qu'un genre en français.
const NOUN_GENDER = {
  f: "barbe bosse carrière chance chips claque clope crème crêpes foudre gloire gorge justice main maison ordonnance ombre ombres poche poches radio radios remarque rues souris trompette vague vagues vapeur voiture fin fins part pub technique auto coke dope donne baskets",
  m: "espace cours geste gestes hymne internet coca malaise plat plastique mai micro office litres mannequin",
};
// Noms invariables que Lexique note seulement au pluriel (« un cours », « une fois »).
const BOTH_NUMBERS = new Set("cours fois".split(" "));
const GENDER_OF = new Map();
for (const [g, list] of Object.entries(NOUN_GENDER)) for (const w of list.split(" ")) GENDER_OF.set(w, g);

// Noms à deux genres (« livre », « livres ») : Lexique ne donne pas non plus leur nombre. On le déduit
// de la terminaison, sinon « trois livre » ne serait pas corrigé : sans -s/-x/-z c'est un singulier ;
// avec, un pluriel si le mot sans sa dernière lettre est aussi un nom, sinon les deux (« souris », « prix »).
const NOMS = new Set();
for (let i = 1; i < lines.length; i++) {
  const f = lines[i].split("\t");
  if (f[C.cgram] === "NOM") NOMS.add(f[C.ortho]);
}
function nounNumber(ortho, nombre) {
  if (BOTH_NUMBERS.has(ortho)) return "";
  if (nombre) return nombre;
  if (!/[sxz]$/.test(ortho)) return "s";
  return NOMS.has(ortho.slice(0, -1)) ? "p" : "";
}

const words = new Map();
for (let i = 1; i < lines.length; i++) {
  const f = lines[i].split("\t");
  if (f.length < head.length - 5) continue;
  const ortho = f[C.ortho];
  const phon = f[C.phon];
  if (!ortho || !phon || ortho.includes(" ")) continue;
  let w = words.get(ortho);
  if (!w) {
    w = { phons: new Set(), freq: 0, tags: new Map(), verbs: new Map(), gn: 0, syll: "" };
    words.set(ortho, w);
  }
  w.phons.add(phon);
  if (!w.syll && f[C.syll]) w.syll = f[C.syll];
  const fr = ((parseFloat(f[C.ff]) || 0) + (parseFloat(f[C.fl]) || 0)) / 2;
  w.freq += fr;
  const cg = f[C.cgram];
  if (cg) w.tags.set(cg, (w.tags.get(cg) || 0) + fr);
  if (cg !== "VER" && cg !== "AUX" || f[C.genre] || f[C.nombre]) {
    const genre = f[C.genre] || (cg === "NOM" && GENDER_OF.get(ortho)) || "";
    if (cg !== "ADV" && cg !== "PRE" && cg !== "CON" && cg !== "ONO") w.gn |= gnMask(genre, cg === "NOM" ? nounNumber(ortho, f[C.nombre]) : f[C.nombre]);
  }
  if ((cg === "VER" || cg === "AUX") && f[C.infover]) {
    const lem = f[C.lemme];
    let v = w.verbs.get(lem);
    if (!v) w.verbs.set(lem, (v = { freq: 0, items: new Set() }));
    v.freq += fr + 0.001;
    for (const iv of f[C.infover].split(";")) if (iv) v.items.add(iv);
  }
}

const rows = [];
for (const [ortho, w] of words) {
  const verbs = [...w.verbs]
    .sort((a, b) => b[1].freq - a[1].freq)
    .map(([lem, v]) => `${lem}=${[...v.items].join(";")}`)
    .join("|");
  rows.push([
    ortho, [...w.phons].join("|"), w.freq.toFixed(2),
    [...w.tags].sort((a, b) => b[1] - a[1]).map(([t]) => t).join(","),
    verbs, w.gn.toString(16), w.syll,
  ].join("\t"));
}
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, rows.join("\n") + "\n", "utf8");
console.log(`${rows.length} mots -> ${out}`);
