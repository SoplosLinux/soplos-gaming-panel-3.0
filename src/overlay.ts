import "./styles/global.css";
import { initI18n } from "./i18n";
import { initTheme } from "./lib/theme";
import { getSettings, type Settings } from "./lib/api";
import { setTempUnit } from "./lib/units";
import Overlay from "./views/Overlay.svelte";

// The overlay: a small surface of the program that stays over the game. It
// reads the same settings as the window, so it speaks the same language and
// follows the same theme.
if (!import.meta.env.DEV) {
  document.addEventListener("contextmenu", (e) => e.preventDefault());
}

async function start() {
  let saved: Settings | null = null;
  try {
    saved = await getSettings();
  } catch {
    // Without the file of settings the overlay opens with the defaults.
  }
  initTheme(saved?.theme ?? null);
  setTempUnit(saved?.tempUnit);
  await initI18n(saved?.language ?? null);
  new Overlay({ target: document.getElementById("app")! });
}

void start();
