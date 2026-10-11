import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

type Suggestion = { word: string; tag: string; info: string };
type Fix = { from: string; to: string; reason: string; kind: string };
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

  useEffect(() => {
    const off = listen<Payload>("suggestions", (e) => {
      setTyped(e.payload.typed);
      setItems(e.payload.items);
      setFix(e.payload.fix);
    });
    return () => {
      off.then((f) => f());
    };
  }, []);

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

      <button className="hide" onClick={() => invoke("toggle")} title="Masquer (Ctrl+Alt+L)">
        ✕
      </button>
    </div>
  );
}
