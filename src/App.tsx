import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { speak } from "./speech";

type Suggestion = { word: string; tag: string; info: string; picto: number | null };
type Fix = { from: string; to: string; reason: string; kind: string; learnable: boolean };
type Notice = { from: string; to: string; reason: string; undone: boolean };
type Payload = { typed: string; items: Suggestion[]; fix: Fix | null };
type UpdateInfo = { state: "available" | "uptodate" | "error" | "downloading"; version: string; message: string; notes: string };
type Settings = { auto: number; audio: boolean; pictos: number };

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
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [audio, setAudio] = useState(false);
  const [pictoMode, setPictoMode] = useState(2);
  // images déjà connues : data URL (dans Scriba), adresse arasaac.org, ou null (pas d'image)
  const images = useRef(new Map<number, string | null>());
  const [, redraw] = useState(0);

  useEffect(() => {
    const off = listen<Payload>("suggestions", (e) => {
      setTyped(e.payload.typed);
      setItems(e.payload.items);
      setFix(e.payload.fix);
    });
    const offAuto = listen<Notice>("auto", (e) => setNotice(e.payload));
    const offUpdate = listen<UpdateInfo>("update", (e) => setUpdate(e.payload));
    const apply = (s: Settings) => {
      setAudio(s.audio);
      setPictoMode(s.pictos);
      images.current.clear();
      redraw((n) => n + 1);
    };
    invoke<Settings>("get_settings").then(apply);
    const offSettings = listen<Settings>("settings", (e) => apply(e.payload));
    return () => {
      off.then((f) => f());
      offAuto.then((f) => f());
      offUpdate.then((f) => f());
      offSettings.then((f) => f());
    };
  }, []);

  // la pastille de correction automatique disparaît toute seule
  useEffect(() => {
    if (!notice) return;
    const id = setTimeout(() => setNotice(null), notice.undone ? 3500 : 6000);
    return () => clearTimeout(id);
  }, [notice]);

  // « à jour » et les erreurs disparaissent seules ; une mise à jour disponible reste jusqu'au clic
  useEffect(() => {
    if (!update || update.state === "available" || update.state === "downloading") return;
    const id = setTimeout(() => setUpdate(null), 6000);
    return () => clearTimeout(id);
  }, [update]);

  const keep = (e: React.MouseEvent) => e.preventDefault();

  function picto(id: number | null): string | null {
    if (id === null || pictoMode === 0) return null;
    const known = images.current.get(id);
    if (known !== undefined) return known;
    images.current.set(id, null);
    invoke<string | null>("picto_data", { id }).then((url) => {
      const remote = `https://static.arasaac.org/pictograms/${id}/${id}_300.png`;
      images.current.set(id, url ?? (pictoMode === 2 ? remote : null));
      redraw((n) => n + 1);
    });
    return null;
  }

  return (
    <div className="bar">
      <div className="grip" data-tauri-drag-region title="Déplacer la barre">
        <span data-tauri-drag-region>Scriba</span>
      </div>

      {fix && (
        <button
          key={"fix:" + fix.from + ">" + fix.to}
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

      {update && (
        <button
          key={"update:" + update.state + update.version}
          className={"update " + update.state}
          onMouseDown={keep}
          disabled={update.state !== "available"}
          onClick={() => {
            invoke("install_update");
          }}
          title={update.notes || update.message}
        >
          <span className="mark">{update.state === "available" ? "⬇" : update.state === "error" ? "!" : update.state === "downloading" ? "…" : "✓"}</span>
          <span className="text">
            <span className="w">{update.state === "available" ? `Scriba ${update.version}` : update.message}</span>
            <span className="meta">{update.state === "available" ? "Cliquer pour installer et redémarrer" : ""}</span>
          </span>
        </button>
      )}

      {notice && (
        <div className={"notice" + (notice.undone ? " undone" : "")} key={"notice:" + notice.from + ">" + notice.to} title={notice.reason}>
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
            onMouseEnter={() => audio && speak(it.word)}
            title={it.info || it.tag}
          >
            <span className="fk">F{i + 1}</span>
            {(() => {
              const src = picto(it.picto);
              return src ? (
                <img className="picto" src={src} alt="" draggable={false} title="Pictogramme ARASAAC (Sergio Palao) · CC BY-NC-SA" />
              ) : null;
            })()}
            <span className="text">
              <span className="w">{it.word}</span>
              <span className="meta">{it.info || it.tag}</span>
            </span>
          </button>
        ))}
      </div>

      <button
        className={"tool" + (audio ? " active" : "")}
        onMouseDown={keep}
        onClick={() => invoke("set_audio", { on: !audio })}
        title={audio ? "Lecture audio activée : passe la souris sur un mot pour l'entendre" : "Activer la lecture audio des mots"}
      >
        <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M11 5 6 9H3v6h3l5 4z" />
          {audio ? <path d="M15.5 8.5a5 5 0 0 1 0 7M18.5 5.5a9 9 0 0 1 0 13" /> : <path d="m16 9 5 6m0-6-5 6" />}
        </svg>
      </button>
      <button className="tool" onMouseDown={keep} onClick={() => invoke("open_reader")} title="Lecture du texte sélectionné (Ctrl+Alt+R)">
        <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M2 5h6a4 4 0 0 1 4 4v11a3 3 0 0 0-3-3H2z" />
          <path d="M22 5h-6a4 4 0 0 0-4 4v11a3 3 0 0 1 3-3h7z" />
        </svg>
      </button>
      <button className="tool" onMouseDown={keep} onClick={() => invoke("open_conj")} title="Conjugaison (Ctrl+Alt+C)">
        <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H20v15H6.5A2.5 2.5 0 0 0 4 20.5z" />
          <path d="M4 20.5A2.5 2.5 0 0 0 6.5 23H20v-5" />
          <path d="M9 8h7M9 12h5" />
        </svg>
      </button>
      <button className="tool" onMouseDown={keep} onClick={() => invoke("open_calc")} title="Calculatrice (Ctrl+Alt+K)">
        <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <rect x="5" y="2.5" width="14" height="19" rx="2.5" />
          <path d="M8.5 6.5h7" />
          <path d="M8.5 11h.01M12 11h.01M15.5 11h.01M8.5 14.5h.01M12 14.5h.01M15.5 14.5h.01M8.5 18h.01M12 18h.01M15.5 18h.01" />
        </svg>
      </button>
      <button className="hide" onClick={() => invoke("toggle")} title="Masquer (Ctrl+Alt+L)">
        ✕
      </button>
    </div>
  );
}
