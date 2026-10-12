import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./conjugation.css";

type Row = { person: number; pronoun: string; aux: string; form: string };
type Tense = { mode: string; name: string; tip: string; rows: Row[] };
type Table = {
  verb: string;
  group: string;
  aux: string;
  present_participle: string;
  past_participle: string;
  tenses: Tense[];
};
type Choice = { verb: string; hint: string };

// Terminaisons à colorier, par temps et par personne (la plus longue qui correspond)
const ENDINGS: Record<string, string[][]> = {
  "Indicatif|Présent": [["e", "s", "x"], ["es", "s", "x"], ["e", "t", "d", "a"], ["ons"], ["ez", "es"], ["ent", "ont"]],
  "Indicatif|Imparfait": [["ais"], ["ais"], ["ait"], ["ions"], ["iez"], ["aient"]],
  "Indicatif|Futur simple": [["ai"], ["as"], ["a"], ["ons"], ["ez"], ["ont"]],
  "Indicatif|Passé simple": [
    ["ai", "is", "us", "ins"],
    ["as", "is", "us", "ins"],
    ["a", "it", "ut", "int"],
    ["âmes", "îmes", "ûmes", "înmes"],
    ["âtes", "îtes", "ûtes", "întes"],
    ["èrent", "irent", "urent", "inrent"],
  ],
  "Conditionnel|Présent": [["ais"], ["ais"], ["ait"], ["ions"], ["iez"], ["aient"]],
  "Subjonctif|Présent": [["e", "s"], ["es", "s"], ["e", "t"], ["ions", "yons"], ["iez", "yez"], ["ent"]],
  "Impératif|Présent": [["e", "s", "a"], ["ons"], ["ez", "es"]],
};

function split(form: string, tense: Tense, person: number): [string, string] {
  const list = ENDINGS[`${tense.mode}|${tense.name}`]?.[person] ?? [];
  const end = list.filter((e) => form.endsWith(e) && form.length > e.length).sort((a, b) => b.length - a.length)[0];
  return end ? [form.slice(0, -end.length), end] : [form, ""];
}

const MODES = ["Indicatif", "Conditionnel", "Subjonctif", "Impératif"];

export default function Conjugation({ initial }: { initial: string }) {
  const [query, setQuery] = useState(initial);
  const [choices, setChoices] = useState<Choice[]>([]);
  const [active, setActive] = useState(0);
  const [open, setOpen] = useState(false);
  const [table, setTable] = useState<Table | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const win = getCurrentWindow();

  const load = async (verb: string) => {
    const t = await invoke<Table | null>("conj_table", { verb });
    if (t) {
      setTable(t);
      setQuery(t.verb);
      setOpen(false);
    }
  };

  useEffect(() => {
    if (initial) load(initial);
    input.current?.focus();
    input.current?.select();
    const off = listen<string>("conj-open", (e) => {
      load(e.payload);
      input.current?.focus();
    });
    const esc = (e: KeyboardEvent) => e.key === "Escape" && win.close();
    window.addEventListener("keydown", esc);
    return () => {
      off.then((f) => f());
      window.removeEventListener("keydown", esc);
    };
  }, []);

  // propositions pendant la frappe
  useEffect(() => {
    if (!open) return;
    const id = setTimeout(async () => {
      setChoices(await invoke<Choice[]>("conj_search", { query }));
      setActive(0);
    }, 60);
    return () => clearTimeout(id);
  }, [query, open]);

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      setActive((a) => Math.min(a + 1, choices.length - 1));
      e.preventDefault();
    } else if (e.key === "ArrowUp") {
      setActive((a) => Math.max(a - 1, 0));
      e.preventDefault();
    } else if (e.key === "Enter" && choices[active]) {
      load(choices[active].verb);
    }
  };

  return (
    <div className="conj">
      <header className="titlebar" data-tauri-drag-region>
        <span className="title" data-tauri-drag-region>
          Conjugaison
        </span>
        <div className="search">
          <input
            ref={input}
            value={query}
            placeholder="Un verbe : manger, finir, ou même « mangeaient »…"
            onChange={(e) => {
              setQuery(e.target.value);
              setOpen(true);
            }}
            onFocus={() => query && setOpen(true)}
            onBlur={() => setTimeout(() => setOpen(false), 120)}
            onKeyDown={onKey}
            spellCheck={false}
          />
          {open && choices.length > 0 && (
            <ul className="choices">
              {choices.map((c, i) => (
                <li
                  key={c.verb}
                  className={i === active ? "active" : ""}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    load(c.verb);
                  }}
                >
                  <span>{c.verb}</span>
                  {c.hint && <small>{c.hint}</small>}
                </li>
              ))}
            </ul>
          )}
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

      {!table && (
        <div className="empty">
          <p>Tape un verbe à l'infinitif, ou une forme que tu as écrite.</p>
          <p className="muted">Astuce : dans n'importe quelle application, Ctrl+Alt+C ouvre ce tableau sur le dernier verbe que tu as écrit.</p>
        </div>
      )}

      {table && (
        <main>
          <section className="verb-head">
            <h1>{table.verb}</h1>
            <div className="facts">
              <span className="pill">{table.group}</span>
              <span className="pill">
                auxiliaire <b>{table.aux}</b>
              </span>
              {table.past_participle && (
                <span className="pill">
                  participe passé <b>{table.past_participle}</b>
                </span>
              )}
              {table.present_participle && (
                <span className="pill">
                  participe présent <b>{table.present_participle}</b>
                </span>
              )}
            </div>
          </section>

          {MODES.map((mode) => {
            const tenses = table.tenses.filter((t) => t.mode === mode);
            if (!tenses.length) return null;
            return (
              <section key={mode} className="mode">
                <h2>{mode}</h2>
                <div className="grid">
                  {tenses.map((t) => (
                    <article key={t.name} className={"tense" + (t.rows[0]?.aux ? " compound" : "")}>
                      <h3>{t.name}</h3>
                      {t.tip && <p className="tip">{t.tip}</p>}
                      <ul>
                        {t.rows.map((r, i) => {
                          const [stem, end] = split(r.form, t, r.person);
                          return (
                            <li key={i}>
                              <span className="pron">{r.pronoun}</span>
                              {r.aux && <span className="aux">{r.aux}</span>}
                              <span className="form">
                                {stem}
                                <em>{end}</em>
                              </span>
                            </li>
                          );
                        })}
                      </ul>
                    </article>
                  ))}
                </div>
              </section>
            );
          })}
          <p className="credit">Conjugaisons : Verbiste (Pierre Sarrazin, licence GPL)</p>
        </main>
      )}
    </div>
  );
}
