import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { setSettings } from "./api";

export type ThemeName = "dark" | "light";

// What the user chose: null follows the theme of the desktop.
const POLL_MS = 3000;

let choice: ThemeName | null = null;
let desktopTheme: ThemeName = "dark";

/** The theme the window is showing right now. */
export const theme = writable<ThemeName>("dark");

function apply() {
  const effective = choice ?? desktopTheme;
  document.documentElement.classList.toggle("light", effective === "light");
  theme.set(effective);
}

async function readDesktopTheme() {
  try {
    const found = await invoke<string>("get_system_theme");
    if (found === "dark" || found === "light") {
      desktopTheme = found;
      apply();
    }
  } catch {
    // Without an answer the last known theme stays.
  }
}

/** Reads the theme of the desktop again, for when the program has just changed it. */
export const refreshTheme = readDesktopTheme;

/** Takes the theme saved in config.json; the desktop theme is read after it. */
export function initTheme(saved: ThemeName | null = null) {
  choice = saved;
  apply();
  void readDesktopTheme();
  // The desktop theme can change while the window is open.
  setInterval(readDesktopTheme, POLL_MS);
}

/** What the user chose: a fixed theme, or null to follow the desktop. */
export function currentChoice(): ThemeName | null {
  return choice;
}

/** Sets the choice from the settings: follow the desktop, light or dark. */
export function setThemeChoice(next: ThemeName | null, save = true) {
  choice = next;
  apply();
  if (save) void setSettings({ theme: choice }).catch(() => {});
}

/**
 * Switches between dark and light. Switching to the theme the desktop already
 * has goes back to following the desktop instead of saving a fixed choice.
 */
export function toggleTheme() {
  const next: ThemeName = get(theme) === "dark" ? "light" : "dark";
  choice = next === desktopTheme ? null : next;
  apply();
  void setSettings({ theme: choice }).catch(() => {});
}
