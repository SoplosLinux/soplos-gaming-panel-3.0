import "./styles/global.css";
import { initI18n } from "./i18n";
import { initTheme } from "./lib/theme";
import { getSettings, type Settings } from "./lib/api";
import App from "./App.svelte";

// This is a desktop program, not a web page: the context menu of the web
// engine (Back, Forward, Reload, Inspect) has no place in it. It stays
// available while the program is run for development.
if (!import.meta.env.DEV) {
  document.addEventListener("contextmenu", (e) => e.preventDefault());
}

// The settings are read once, before the first render, so the window opens
// with the theme and the language the user chose and never flashes another.
async function start() {
  let saved: Settings | null = null;
  try {
    saved = await getSettings();
  } catch {
    // Without the file of settings the program opens with the defaults.
  }
  initTheme(saved?.theme ?? null);
  await initI18n(saved?.language ?? null);
  new App({ target: document.getElementById("app")! });
}

void start();
