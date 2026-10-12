import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { onVoices, saveVoice, savedVoice, speak, stopSpeaking } from "./speech";
import "./reader.css";

type Piece = { text: string; syll: string[] };

type Prefs = {
  size: number;
  font: string;
  letter: number;
  word: number;
  line: number;
  syllables: boolean;
  ruler: boolean;
  theme: "blanc" | "creme" | "gris" | "sombre";
  rate: number;
};

const DEFAULTS: Prefs = {
  size: 26,
  font: "Verdana",
  letter: 0.04,
  word: 0.18,
  line: 1.9,
  syllables: true,
  ruler: false,
  theme: "creme",
  rate: 0.95,
};

const FONTS = ["Verdana", "Arial", "Segoe UI", "Comic Sans MS", "Century Gothic", "Tahoma", "OpenDyslexic"];
const THEMES: [Prefs["theme"], string][] = [
  ["blanc", "Blanc"],
  ["creme", "Crème"],
  ["gris", "Gris"],
  ["sombre", "Sombre"],
];
const PREFS_KEY = "scriba.reader";

function loadPrefs(): Prefs {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}") };
  } catch {
    return DEFAULTS;
  }
}

/** Morceaux de lecture : une phrase (ou un paragraphe) à la fois, pour une voix fluide. */
function chunks(pieces: Piece[]): { from: number; to: number }[] {
  const out: { from: number; to: number }[] = [];
  let from = 0;
  pieces.forEach((p, i) => {
    if (!p.syll.length && /[.!?;:\n]/.test(p.text)) {
      out.push({ from, to: i + 1 });
      from = i + 1;
    }
  });
  if (from < pieces.length) out.push({ from, to: pieces.length });
  return out;
}

export default function Reader() {
  const [text, setText] = useState("");
  const [pieces, setPieces] = useState<Piece[]>([]);
  const [prefs, setPrefs] = useState<Prefs>(loadPrefs);
  const [panel, setPanel] = useState(false);
  const [editing, setEditing] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [current, setCurrent] = useState(-1);
  const [voices, setVoices] = useState<SpeechSynthesisVoice[]>([]);
  const [voice, setVoice] = useState(savedVoice());
  const [rulerY, setRulerY] = useState<number | null>(null);
  const page = useRef<HTMLDivElement>(null);
  const stopRef = useRef(false);
  const win = getCurrentWindow();
  const latest = useRef({ playing, editing, play: (_?: number) => {}, stop: () => {} });

  // texte reçu de Scriba (sélection ou phrase en cours)
  useEffect(() => {
    invoke<string>("reading_text").then(setText);
    const off = listen<string>("reader-text", (e) => {
      stop();
      setEditing(false);
      setText(e.payload);
    });
    const offVoices = onVoices(setVoices);
    const keys = (e: KeyboardEvent) => {
      const l = latest.current;
      if (e.key === "Escape") win.close();
      if (e.key === " " && !l.editing && (e.target as HTMLElement).tagName !== "TEXTAREA") {
        e.preventDefault();
        l.playing ? l.stop() : l.play();
      }
    };
    window.addEventListener("keydown", keys);
    return () => {
      off.then((f) => f());
      offVoices();
      window.removeEventListener("keydown", keys);
      stopSpeaking();
    };
  }, []);

  useEffect(() => {
    invoke<Piece[]>("syllabify", { text }).then(setPieces);
  }, [text]);

  useEffect(() => {
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
    } catch {
      /* préférences non enregistrées, sans gravité */
    }
  }, [prefs]);

  // début (en caractères) de chaque morceau dans le texte
  const offsets = useMemo(() => {
    const o: number[] = [];
    let n = 0;
    for (const p of pieces) {
      o.push(n);
      n += p.text.length;
    }
    return o;
  }, [pieces]);

  const set = <K extends keyof Prefs>(k: K, v: Prefs[K]) => setPrefs((p) => ({ ...p, [k]: v }));

  function play(startAt = 0) {
    const parts = chunks(pieces).filter((c) => c.to > startAt);
    if (!parts.length) return;
    stopRef.current = false;
    setPlaying(true);
    const next = (k: number) => {
      if (stopRef.current || k >= parts.length) {
        setPlaying(false);
        setCurrent(-1);
        return;
      }
      const from = Math.max(parts[k].from, startAt);
      const slice = pieces.slice(from, parts[k].to);
      const base = offsets[from] ?? 0;
      speak(slice.map((p) => p.text).join(""), {
        rate: prefs.rate,
        voice,
        onWord: (ci) => {
          const pos = base + ci;
          let idx = from;
          while (idx + 1 < pieces.length && offsets[idx + 1] <= pos) idx++;
          setCurrent(idx);
        },
        onEnd: () => next(k + 1),
      });
    };
    next(0);
  }

  function stop() {
    stopRef.current = true;
    stopSpeaking();
    setPlaying(false);
    setCurrent(-1);
  }

  latest.current = { playing, editing, play, stop };

  async function paste() {
    stop();
    setEditing(false);
    setText(await invoke<string>("clipboard_text"));
  }

  const style = {
    fontSize: `${prefs.size}px`,
    fontFamily: `"${prefs.font}", Verdana, sans-serif`,
    letterSpacing: `${prefs.letter}em`,
    wordSpacing: `${prefs.word}em`,
    lineHeight: prefs.line,
  } as React.CSSProperties;

  return (
    <div className={`reader theme-${prefs.theme}`}>
      <header className="titlebar" data-tauri-drag-region>
        <span className="title" data-tauri-drag-region>
          Lecture
        </span>
        <div className="tools">
          {playing ? (
            <button className="primary" onClick={stop} title="Arrêter (Espace)">
              ■ Arrêter
            </button>
          ) : (
            <button className="primary" onClick={() => play()} disabled={!pieces.length} title="Lire à voix haute (Espace)">
              ▶ Lire
            </button>
          )}
          <label className="rate" title="Vitesse de lecture">
            <span>Vitesse</span>
            <input type="range" min={0.6} max={1.4} step={0.05} value={prefs.rate} onChange={(e) => set("rate", +e.target.value)} />
          </label>
          <button onClick={() => setPanel((p) => !p)} className={panel ? "on" : ""} title="Taille, police, espacement…">
            Aa
          </button>
          <button onClick={() => { stop(); setEditing((x) => !x); }} className={editing ? "on" : ""} title="Modifier le texte">
            Modifier
          </button>
          <button onClick={paste} title="Lire le texte copié">
            Coller
          </button>
        </div>
        <div className="win-buttons">
          <button onClick={() => win.minimize()} title="Réduire">
            –
          </button>
          <button className="close" onClick={() => win.close()} title="Fermer (Échap)">
            ✕
          </button>
        </div>
      </header>

      {panel && (
        <section className="panel">
          <label>
            Taille
            <input type="range" min={16} max={48} value={prefs.size} onChange={(e) => set("size", +e.target.value)} />
          </label>
          <label>
            Police
            <select value={prefs.font} onChange={(e) => set("font", e.target.value)}>
              {FONTS.map((f) => (
                <option key={f} value={f} style={{ fontFamily: f }}>
                  {f}
                </option>
              ))}
            </select>
          </label>
          <label>
            Lettres
            <input type="range" min={0} max={0.3} step={0.01} value={prefs.letter} onChange={(e) => set("letter", +e.target.value)} />
          </label>
          <label>
            Mots
            <input type="range" min={0} max={0.8} step={0.02} value={prefs.word} onChange={(e) => set("word", +e.target.value)} />
          </label>
          <label>
            Lignes
            <input type="range" min={1.3} max={2.8} step={0.05} value={prefs.line} onChange={(e) => set("line", +e.target.value)} />
          </label>
          <label className="check">
            <input type="checkbox" checked={prefs.syllables} onChange={(e) => set("syllables", e.target.checked)} />
            Syllabes en couleur
          </label>
          <label className="check">
            <input type="checkbox" checked={prefs.ruler} onChange={(e) => set("ruler", e.target.checked)} />
            Règle de lecture
          </label>
          <label>
            Fond
            <select value={prefs.theme} onChange={(e) => set("theme", e.target.value as Prefs["theme"])}>
              {THEMES.map(([v, n]) => (
                <option key={v} value={v}>
                  {n}
                </option>
              ))}
            </select>
          </label>
          {voices.length > 0 && (
            <label>
              Voix
              <select
                value={voice || voices[0]?.name}
                onChange={(e) => {
                  setVoice(e.target.value);
                  saveVoice(e.target.value);
                }}
              >
                {voices.map((v) => (
                  <option key={v.name} value={v.name}>
                    {v.name.replace(/^Microsoft /, "").replace(/ - .*$/, "")}
                  </option>
                ))}
              </select>
            </label>
          )}
        </section>
      )}

      {editing ? (
        <textarea
          className="editor"
          style={style}
          value={text}
          autoFocus
          spellCheck={false}
          onChange={(e) => setText(e.target.value)}
          placeholder="Écris ou colle un texte ici…"
        />
      ) : (
        <div
          className="page"
          ref={page}
          style={style}
          onMouseMove={(e) => {
            if (!prefs.ruler || !page.current) return;
            const r = page.current.getBoundingClientRect();
            setRulerY(e.clientY - r.top + page.current.scrollTop);
          }}
          onMouseLeave={() => setRulerY(null)}
        >
          {prefs.ruler && rulerY !== null && (
            <div className="ruler" style={{ top: rulerY - (prefs.size * prefs.line) / 2, height: prefs.size * prefs.line }} />
          )}
          {!pieces.length && (
            <p className="empty">
              Sélectionne un texte dans n'importe quelle application, puis appuie sur <b>Ctrl+Alt+R</b>.
              <br />
              Tu peux aussi le copier et cliquer sur <b>Coller</b>, ou l'écrire avec ✎.
            </p>
          )}
          {pieces.map((p, i) =>
            p.syll.length ? (
              <span
                key={i}
                className={"word" + (i === current ? " speaking" : "")}
                onClick={() => (playing ? (stop(), play(i)) : speak(p.text, { rate: prefs.rate, voice }))}
                title="Clique pour écouter ce mot"
              >
                {prefs.syllables && p.syll.length > 1
                  ? p.syll.map((s, k) => (
                      <span key={k} className={`sy${k % 2}`}>
                        {s}
                      </span>
                    ))
                  : p.text}
              </span>
            ) : (
              <span key={i}>{p.text}</span>
            ),
          )}
        </div>
      )}
    </div>
  );
}
