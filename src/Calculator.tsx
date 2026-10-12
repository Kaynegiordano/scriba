import { useCallback, useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { evaluate, formatNumber, numberWords } from "./calc";
import { speak, stopSpeaking } from "./speech";
import "./calculator.css";

type Line = { expr: string; result: string };
type Prefs = { font: "dys" | "std"; theme: "creme" | "sombre"; pinned: boolean };

const PREFS_KEY = "scriba.calc";
const DEFAULTS: Prefs = { font: "dys", theme: "creme", pinned: true };

function loadPrefs(): Prefs {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}") };
  } catch {
    return DEFAULTS;
  }
}

const KEYS: { label: string; send: string; kind: string; title?: string; wide?: boolean }[] = [
  { label: "√", send: "√", kind: "fn", title: "Racine carrée : √16 = 4" },
  { label: "x²", send: "^2", kind: "fn", title: "Au carré" },
  { label: "(", send: "(", kind: "fn" },
  { label: ")", send: ")", kind: "fn" },
  { label: "C", send: "C", kind: "clear", title: "Tout effacer (Échap)" },
  { label: "⌫", send: "⌫", kind: "fn", title: "Effacer le dernier caractère (Retour arrière)" },
  { label: "%", send: "%", kind: "fn", title: "Pourcentage : 20% = 0,2" },
  { label: "÷", send: "÷", kind: "op" },
  { label: "7", send: "7", kind: "num" },
  { label: "8", send: "8", kind: "num" },
  { label: "9", send: "9", kind: "num" },
  { label: "×", send: "×", kind: "op" },
  { label: "4", send: "4", kind: "num" },
  { label: "5", send: "5", kind: "num" },
  { label: "6", send: "6", kind: "num" },
  { label: "−", send: "−", kind: "op" },
  { label: "1", send: "1", kind: "num" },
  { label: "2", send: "2", kind: "num" },
  { label: "3", send: "3", kind: "num" },
  { label: "+", send: "+", kind: "op" },
  { label: "0", send: "0", kind: "num", wide: true },
  { label: ",", send: ",", kind: "num" },
  { label: "=", send: "=", kind: "equal", title: "Calculer (Entrée)" },
];

/** Colorie l'expression : chiffres, signes, parenthèses. */
function Colored({ text }: { text: string }) {
  return (
    <>
      {[...text].map((c, i) => (
        <span key={i} className={/[0-9]/.test(c) ? "d" : /[,.]/.test(c) ? "c" : /[()]/.test(c) ? "p" : "o"}>
          {c}
        </span>
      ))}
    </>
  );
}

export default function Calculator() {
  const [prefs, setPrefs] = useState<Prefs>(loadPrefs);
  const [expr, setExpr] = useState("");
  const [fresh, setFresh] = useState(false); // le résultat vient d'être affiché
  const [history, setHistory] = useState<Line[]>([]);
  const [error, setError] = useState("");
  const [speaking, setSpeaking] = useState(false);
  const win = getCurrentWindow();

  const setPref = <K extends keyof Prefs>(k: K, v: Prefs[K]) =>
    setPrefs((p) => {
      const next = { ...p, [k]: v };
      try {
        localStorage.setItem(PREFS_KEY, JSON.stringify(next));
      } catch {
        /* préférence non enregistrée, sans gravité */
      }
      return next;
    });

  useEffect(() => {
    win.setAlwaysOnTop(prefs.pinned).catch(() => {});
  }, [prefs.pinned, win]);

  const live = useMemo(() => {
    const r = evaluate(expr);
    return r.ok ? r : null;
  }, [expr]);

  const press = useCallback(
    (key: string) => {
      setError("");
      if (key === "C") {
        setExpr("");
        setFresh(false);
        stopSpeaking();
        setSpeaking(false);
        return;
      }
      if (key === "⌫") {
        setExpr((e) => (fresh ? "" : e.slice(0, -1)));
        setFresh(false);
        return;
      }
      if (key === "=") {
        if (!expr) return;
        const r = evaluate(expr);
        if (!r.ok) {
          setError(r.error || "Expression incomplète");
          return;
        }
        const text = formatNumber(r.value);
        setHistory((h) => [{ expr, result: text }, ...h].slice(0, 30));
        setExpr(text.replace(/ /g, ""));
        setFresh(true);
        return;
      }
      const startsNumber = /[0-9,]/.test(key) || key === "(" || key === "√";
      setExpr((e) => {
        // après « = » : un chiffre démarre un nouveau calcul, un signe continue avec le résultat
        const base = fresh && startsNumber ? "" : e;
        // deux signes de suite : le second remplace le premier
        if (/^[+×÷]$/.test(key) && /[+×÷−]$/.test(base) && base.length > 1) return base.slice(0, -1) + key;
        // une seule virgule par nombre
        if (key === "," && /[0-9]*,[0-9]*$/.test(base)) return base;
        return base + key;
      });
      setFresh(false);
    },
    [expr, fresh],
  );

  // Clavier : chiffres, signes, Entrée, Retour arrière, Échap
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      const map: Record<string, string> = {
        "*": "×",
        x: "×",
        "/": "÷",
        "-": "−",
        ".": ",",
        Enter: "=",
        "=": "=",
        Backspace: "⌫",
        Escape: "C",
        Delete: "C",
        "²": "^2",
      };
      const k = map[e.key] ?? e.key;
      if (/^[0-9+×÷−,()%^√=C⌫]$/.test(k) || k === "^2") {
        e.preventDefault();
        press(k);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [press]);

  // Le résultat s'affiche dès qu'il y a un vrai calcul (pas pour un simple nombre tapé)
  const hasOperation = /[+×÷−^%√(]/.test(expr.slice(1));
  const showResult = !!live && (fresh || hasOperation);
  const words = live && showResult ? numberWords(live.value) : "";
  const resultText = live ? formatNumber(live.value) : "";

  const read = () => {
    if (speaking) {
      stopSpeaking();
      setSpeaking(false);
      return;
    }
    if (!words) return;
    setSpeaking(true);
    speak(words, { onEnd: () => setSpeaking(false) });
  };

  return (
    <div className={`calc theme-${prefs.theme} font-${prefs.font}`}>
      <header className="titlebar" data-tauri-drag-region>
        <span className="title" data-tauri-drag-region>
          Calculatrice
        </span>
        <div className="tools">
          <button onClick={() => setPref("font", prefs.font === "dys" ? "std" : "dys")} className={prefs.font === "dys" ? "on" : ""} title="Police OpenDyslexic">
            Aa
          </button>
          <button onClick={() => setPref("theme", prefs.theme === "creme" ? "sombre" : "creme")} title="Fond crème / sombre">
            {prefs.theme === "creme" ? "☾" : "☀"}
          </button>
          <button
            onClick={() => setPref("pinned", !prefs.pinned)}
            className={prefs.pinned ? "on" : ""}
            title={prefs.pinned ? "Reste au premier plan (cliquer pour libérer)" : "Garder au premier plan"}
          >
            ⚲
          </button>
          <button onClick={() => win.minimize()} title="Réduire">
            –
          </button>
          <button className="close" onClick={() => win.close()} title="Fermer">
            ✕
          </button>
        </div>
      </header>

      <section className="screen">
        <div className="history">
          {history
            .slice(0, 3)
            .reverse()
            .map((l, i) => (
              <button
                key={i}
                className="past"
                onClick={() => {
                  setExpr(l.expr);
                  setFresh(false);
                }}
                title="Reprendre ce calcul"
              >
                <span className="pe">
                  <Colored text={l.expr} />
                </span>
                <span className="pr">= {l.result}</span>
              </button>
            ))}
        </div>
        <div className="expr" aria-live="polite">
          {expr ? <Colored text={expr} /> : <span className="hint">0</span>}
        </div>
        <div className={"result" + (error ? " error" : "")}>{error || (showResult ? "= " + resultText : " ")}</div>
        <div className="words">
          <span>{words || " "}</span>
          {words && (
            <button className="speak" onClick={read} title={speaking ? "Arrêter" : "Lire le résultat à voix haute"}>
              {speaking ? "■" : "▶"}
            </button>
          )}
        </div>
      </section>

      <section className="keys">
        {KEYS.map((k) => (
          <button key={k.label} className={`key ${k.kind}${k.wide ? " wide" : ""}`} title={k.title} onClick={() => press(k.send)}>
            {k.label}
          </button>
        ))}
      </section>
    </div>
  );
}
