import { createRoot } from "react-dom/client";
import App from "./App";
import Conjugation from "./Conjugation";
import "./styles.css";

// Même interface pour les deux fenêtres : la barre et la conjugaison (#conj=verbe)
const hash = decodeURIComponent(window.location.hash.slice(1));
const root = createRoot(document.getElementById("root")!);

if (hash.startsWith("conj")) {
  document.documentElement.classList.add("conj-page");
  root.render(<Conjugation initial={hash.split("=")[1] ?? ""} />);
} else {
  root.render(<App />);
}
