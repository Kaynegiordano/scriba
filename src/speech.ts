// Synthèse vocale : les voix françaises installées dans Windows (Hortense, Julie, Paul…).

const VOICE_KEY = "scriba.voice";

export function frenchVoices(): SpeechSynthesisVoice[] {
  return speechSynthesis.getVoices().filter((v) => v.lang.toLowerCase().startsWith("fr"));
}

/** Les voix arrivent parfois après le chargement de la page. */
export function onVoices(cb: (voices: SpeechSynthesisVoice[]) => void): () => void {
  const run = () => cb(frenchVoices());
  run();
  speechSynthesis.addEventListener("voiceschanged", run);
  return () => speechSynthesis.removeEventListener("voiceschanged", run);
}

export function savedVoice(): string {
  try {
    return localStorage.getItem(VOICE_KEY) ?? "";
  } catch {
    return "";
  }
}

export function saveVoice(name: string) {
  try {
    localStorage.setItem(VOICE_KEY, name);
  } catch {
    /* préférence non enregistrée, sans gravité */
  }
}

function voiceFor(name: string): SpeechSynthesisVoice | undefined {
  const voices = frenchVoices();
  return voices.find((v) => v.name === name) ?? voices[0];
}

type Options = {
  rate?: number;
  voice?: string;
  /** position (en caractères dans `text`) du mot en train d'être lu */
  onWord?: (charIndex: number) => void;
  onEnd?: () => void;
};

export function speak(text: string, opts: Options = {}) {
  speechSynthesis.cancel();
  if (!text.trim()) return;
  const u = new SpeechSynthesisUtterance(text);
  u.lang = "fr-FR";
  const v = voiceFor(opts.voice ?? savedVoice());
  if (v) u.voice = v;
  u.rate = opts.rate ?? 1;
  if (opts.onWord) u.onboundary = (e) => e.name === "word" && opts.onWord!(e.charIndex);
  if (opts.onEnd) {
    u.onend = opts.onEnd;
    u.onerror = opts.onEnd;
  }
  speechSynthesis.speak(u);
}

export function stopSpeaking() {
  speechSynthesis.cancel();
}
