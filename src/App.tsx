import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

type Suggestion = { word: string; tag: string; info: string };
type Fix = { from: string; to: string; reason: string; kind: string; learnable: boolean };
type Notice = { from: string; to: string; reason: string; undone: boolean };
type Payload = { typed: string; items: Suggestion[]; fix: Fix | null };

const KIND_LABEL: Record<string, string> = {
  orthographe: "Orthographe",
  accord: "Accord",
  verbe: "Verbe",
  homophone: "Homophone",
};

export default function App() {
  const [typed, setTyped] = useState("");
  const [items, setItems] = useState<Suggestion[]>([]);
  const [fix, setFix] = useState<Fix | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);

  useEffect(() => {
    const off = listen<Payload>("suggestions", (e) => {
      setTyped(e.payload.typed);
      setItems(e.payload.items);
      setFix(e.payload.fix);
    });
    const offAuto = listen<Notice>("auto", (e) => setNotice(e.payload));
    return () => {
      off.then((f) => f());
      offAuto.then((f) => f());
    };
  }, []);

  // la pastille de correction automatique disparaît toute seule
  useEffect(() => {
    if (!notice) return;
    const id = setTimeout(() => setNotice(null), notice.undone ? 3500 : 6000);
    return () => clearTimeout(id);
  }, [notice]);

  const keep = (e: React.MouseEvent) => e.preventDefault();

  return (
    <div className="bar">
      <div className="grip" data-tauri-drag-region title="Déplacer la barre">
        <span data-tauri-drag-region>Scriba</span>
      </div>

      {fix && (
        <button
          key={fix.from + fix.to}
          className={"fix " + fix.kind}
          onMouseDown={keep}
          onClick={() => invoke("apply_fix")}
          title={fix.reason}
        >
          <span className="fk">F7</span>
          <span className="text">
            <span className="w">
              <s>{fix.from}</s> → {fix.to}
            </span>
            <span className="meta">
              {KIND_LABEL[fix.kind] ?? "Correction"} · {fix.reason}
            </span>
          </span>
        </button>
      )}

      {fix?.learnable && (
        <button className="learn" onMouseDown={keep} onClick={() => invoke("learn")} title="Ce mot est correct : ne plus le signaler">
          <span className="fk">F8</span> Mot correct
        </button>
      )}

      {notice && (
        <div className={"notice" + (notice.undone ? " undone" : "")} key={notice.from + notice.to} title={notice.reason}>
          <span className="mark">{notice.undone ? "↩" : "✓"}</span>
          <span className="text">
            <span className="w">
              <s>{notice.from}</s> → {notice.to}
            </span>
            <span className="meta">{notice.undone ? notice.reason : "Corrigé · Retour arrière pour annuler"}</span>
          </span>
        </div>
      )}

      <div className={"typed" + (typed ? "" : " empty")}>{typed || "…"}</div>

      <div className="list">
        {items.length === 0 && !fix && (
          <span className="hint">{typed ? "Aucune proposition" : "Commence à écrire, les mots apparaîtront ici"}</span>
        )}
        {items.map((it, i) => (
          <button
            key={typed + "|" + it.word}
            className={"sugg" + (it.info ? " verb" : "")}
            style={{ animationDelay: `${i * 18}ms` }}
            onMouseDown={keep}
            onClick={() => invoke("pick", { index: i })}
            title={it.info || it.tag}
          >
            <span className="fk">F{i + 1}</span>
            <span className="text">
              <span className="w">{it.word}</span>
              <span className="meta">{it.info || it.tag}</span>
            </span>
          </button>
        ))}
      </div>

      <button className="tool" onMouseDown={keep} onClick={() => invoke("open_conj")} title="Conjugaison (Ctrl+Alt+C)">
        <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H20v15H6.5A2.5 2.5 0 0 0 4 20.5z" />
          <path d="M4 20.5A2.5 2.5 0 0 0 6.5 23H20v-5" />
          <path d="M9 8h7M9 12h5" />
        </svg>
      </button>
      <button className="hide" onClick={() => invoke("toggle")} title="Masquer (Ctrl+Alt+L)">
        ✕
      </button>
    </div>
  );
}
