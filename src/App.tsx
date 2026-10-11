import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

type Suggestion = { word: string; tag: string; info: string };
type Payload = { typed: string; items: Suggestion[] };

export default function App() {
  const [typed, setTyped] = useState("");
  const [items, setItems] = useState<Suggestion[]>([]);

  useEffect(() => {
    const off = listen<Payload>("suggestions", (e) => {
      setTyped(e.payload.typed);
      setItems(e.payload.items);
    });
    return () => {
      off.then((f) => f());
    };
  }, []);

  return (
    <div className="bar">
      <div className="grip" data-tauri-drag-region title="Déplacer la barre">
        <span data-tauri-drag-region>Scriba</span>
      </div>

      <div className={"typed" + (typed ? "" : " empty")}>{typed || "…"}</div>

      <div className="list">
        {items.length === 0 && (
          <span className="hint">{typed ? "Aucune proposition" : "Commence à écrire, les mots apparaîtront ici"}</span>
        )}
        {items.map((it, i) => (
          <button
            key={typed + "|" + it.word}
            className={"sugg" + (it.info ? " verb" : "")}
            style={{ animationDelay: `${i * 18}ms` }}
            onMouseDown={(e) => e.preventDefault()}
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
