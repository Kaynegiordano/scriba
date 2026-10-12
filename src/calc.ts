// Calculatrice de Scriba : évaluation sûre d'une expression (sans eval) et nombres écrits en lettres.

export type Result = { ok: true; value: number } | { ok: false; error: string };

type Tok = { t: "num"; v: number } | { t: "op"; v: string };

function tokenize(src: string): Tok[] | string {
  const out: Tok[] = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (c === " " || c === "\u202f" || c === "\u00a0") {
      i++;
    } else if (/[0-9]/.test(c) || c === "," || c === ".") {
      let j = i;
      while (j < src.length && /[0-9,.]/.test(src[j])) j++;
      const raw = src.slice(i, j).replace(",", ".");
      if ((raw.match(/\./g) ?? []).length > 1 || raw === ".") return "Nombre incorrect";
      out.push({ t: "num", v: parseFloat(raw) });
      i = j;
    } else if ("+−-×*÷/^()%√".includes(c)) {
      out.push({ t: "op", v: c === "-" ? "−" : c === "*" ? "×" : c === "/" ? "÷" : c });
      i++;
    } else {
      return `Caractère inconnu : ${c}`;
    }
  }
  return out;
}

/** Calcule l'expression ; les parenthèses ouvertes sont fermées toutes seules. */
export function evaluate(src: string): Result {
  const toks = tokenize(src);
  if (typeof toks === "string") return { ok: false, error: toks };
  if (!toks.length) return { ok: false, error: "" };
  let open = 0;
  for (const t of toks) if (t.t === "op") open += t.v === "(" ? 1 : t.v === ")" ? -1 : 0;
  for (; open > 0; open--) toks.push({ t: "op", v: ")" });

  let p = 0;
  const peek = () => toks[p];
  const isOp = (v: string) => peek()?.t === "op" && (peek() as { v: string }).v === v;
  const fail = (m: string): never => {
    throw new Error(m);
  };

  const expr = (): number => {
    let a = term();
    while (isOp("+") || isOp("−")) {
      const op = (toks[p++] as { v: string }).v;
      const b = term();
      a = op === "+" ? a + b : a - b;
    }
    return a;
  };
  const term = (): number => {
    let a = unary();
    for (;;) {
      if (isOp("×") || isOp("÷")) {
        const op = (toks[p++] as { v: string }).v;
        const b = unary();
        if (op === "÷" && b === 0) fail("Division par zéro impossible");
        a = op === "×" ? a * b : a / b;
      } else if (isOp("(") || isOp("√")) {
        a *= unary(); // 2(3+4) = 2 × 7
      } else {
        return a;
      }
    }
  };
  const unary = (): number => {
    if (isOp("−")) {
      p++;
      return -unary();
    }
    if (isOp("+")) {
      p++;
      return unary();
    }
    return power();
  };
  const power = (): number => {
    const base = postfix();
    if (isOp("^")) {
      p++;
      return Math.pow(base, unary());
    }
    return base;
  };
  const postfix = (): number => {
    let a = primary();
    while (isOp("%")) {
      p++;
      a /= 100;
    }
    return a;
  };
  const primary = (): number => {
    const t = toks[p];
    if (!t) return fail("Il manque un nombre");
    if (t.t === "num") {
      p++;
      return t.v;
    }
    if (t.v === "(") {
      p++;
      const v = expr();
      if (!isOp(")")) fail("Parenthèse manquante");
      p++;
      return v;
    }
    if (t.v === "√") {
      p++;
      const v = primary();
      if (v < 0) fail("Pas de racine carrée d'un nombre négatif");
      return Math.sqrt(v);
    }
    return fail("Il manque un nombre");
  };

  try {
    const v = expr();
    if (p < toks.length) return { ok: false, error: "Expression incomplète" };
    if (!Number.isFinite(v)) return { ok: false, error: "Résultat trop grand" };
    return { ok: true, value: Number(v.toPrecision(12)) };
  } catch (e) {
    return { ok: false, error: (e as Error).message };
  }
}

/** Écriture décimale simple du nombre (sans bruit de calcul, sans exposant). */
function plain(n: number): string {
  const x = Number(Math.abs(n).toPrecision(12));
  const text = String(x);
  return /e/.test(text) ? x.toFixed(10).replace(/0+$/, "").replace(/\.$/, "") : text;
}

/** 1234.5 -> « 1 234,5 » (espaces fines, virgule). */
export function formatNumber(n: number): string {
  if (Object.is(n, -0)) n = 0;
  const [int, dec] = plain(n).split(".");
  const grouped = int.replace(/\B(?=(\d{3})+(?!\d))/g, "\u202f");
  return (n < 0 ? "−" : "") + grouped + (dec ? "," + dec : "");
}

const UNITS = ["zéro", "un", "deux", "trois", "quatre", "cinq", "six", "sept", "huit", "neuf", "dix", "onze", "douze", "treize", "quatorze", "quinze", "seize"];
const TENS = ["", "", "vingt", "trente", "quarante", "cinquante", "soixante"];

function below100(n: number, final: boolean): string {
  if (n < 17) return UNITS[n];
  if (n < 20) return "dix-" + UNITS[n - 10];
  if (n < 70) {
    const t = Math.floor(n / 10);
    const u = n % 10;
    return TENS[t] + (u === 0 ? "" : u === 1 ? " et un" : "-" + UNITS[u]);
  }
  if (n < 80) return n === 71 ? "soixante et onze" : "soixante-" + below100(n - 60, false);
  if (n === 80) return final ? "quatre-vingts" : "quatre-vingt";
  const r = n - 80;
  return "quatre-vingt-" + (r < 17 ? UNITS[r] : "dix-" + UNITS[r - 10]);
}

function below1000(n: number, final: boolean): string {
  const h = Math.floor(n / 100);
  const r = n % 100;
  if (h === 0) return below100(r, final);
  const cent = h === 1 ? "cent" : UNITS[h] + " cent" + (r === 0 && final ? "s" : "");
  return r === 0 ? cent : cent + " " + below100(r, final);
}

function integerWords(n: number): string {
  if (n === 0) return "zéro";
  const parts: string[] = [];
  const milliards = Math.floor(n / 1e9);
  const millions = Math.floor(n / 1e6) % 1000;
  const milliers = Math.floor(n / 1e3) % 1000;
  const rest = n % 1000;
  if (milliards) parts.push(below1000(milliards, false) + " milliard" + (milliards > 1 ? "s" : ""));
  if (millions) parts.push(below1000(millions, false) + " million" + (millions > 1 ? "s" : ""));
  if (milliers) parts.push(milliers === 1 ? "mille" : below1000(milliers, false) + " mille");
  if (rest) parts.push(below1000(rest, true));
  return parts.join(" ");
}

/** 42.5 -> « quarante-deux virgule cinq » ; vide si le nombre est trop grand. */
export function numberWords(n: number): string {
  if (!Number.isFinite(n) || Math.abs(n) >= 1e12) return "";
  const [int, dec] = plain(n).split(".");
  let words = integerWords(parseInt(int, 10));
  if (dec) {
    const zeros = dec.match(/^0*/)![0].length;
    const tail = dec.slice(zeros);
    words += " virgule" + " zéro".repeat(zeros) + (tail ? " " + integerWords(parseInt(tail, 10)) : "");
  }
  return (n < 0 ? "moins " : "") + words;
}
