import { invoke } from "@tauri-apps/api/core";

export type ThemeChoice = "dark" | "light" | null;

/** Where the small window of the overlay sits on the screen. */
export type HudCorner = "top-left" | "top-right" | "bottom-left" | "bottom-right";

/** The two shapes of the overlay: the card in a corner, or the full-height panel. */
export type HudMode = "simple" | "advanced";

/** The cells the overlay can draw. */
export type HudMetric =
  | "cpu"
  | "cpuTemp"
  | "cpuWatts"
  | "clocks"
  | "gpu"
  | "gpuTemp"
  | "vram"
  | "ram"
  | "fps"
  | "frametime"
  | "net"
  | "disk"
  | "fans"
  | "voltage"
  | "battery"
  | "scheduler"
  | "profile"
  | "session"
  | "clock";

/** What the overlay draws. */
export interface HudSettings {
  enabled: boolean;
  mode: HudMode;
  corner: HudCorner;
  /** 0-100. */
  opacity: number;
  /** Percentage of the size the interface was designed at. */
  scale: number;
  /** The cells to draw, in the order of the list. Empty draws none. */
  metrics: HudMetric[];
  /** Shown when a game starts and hidden when it ends. */
  autoShow: boolean;
  /** Milliseconds between two readings: 500, 1000 or 2000. */
  refreshMs: number;
  /** Degrees Celsius from which the figure turns red; 0 never. */
  cpuAlert: number;
  gpuAlert: number;
}

export interface Settings {
  theme: ThemeChoice;
  /** null follows the language of the desktop. */
  language: string | null;
  panelHotkey: string;
  hudHotkey: string;
  startWithSession: boolean;
  /** Started by the session, the panel stays in the tray. */
  startMinimized: boolean;
  /** Closing the window leaves the panel in the tray; off, it quits. */
  closeToTray: boolean;
  notifyGameMode: boolean;
  tempUnit: "c" | "f";
  hud: HudSettings;
}

export const settingsExport = (path: string) => invoke<void>("settings_export", { path });
export const settingsImport = (path: string) => invoke<Settings>("settings_import", { path });
export const settingsReset = () => invoke<Settings>("settings_reset");
export const historyClear = () => invoke<void>("history_clear");

export const getSettings = () => invoke<Settings>("get_settings");
/** Changes only the keys given; returns the settings as they stay saved. */
export const setSettings = (patch: Partial<Settings>) => invoke<Settings>("set_settings", { patch });
export const configPath = () => invoke<string>("config_path");
/** Registers the shortcuts of config.json again (after the user changed them). */
export const applyHotkeys = () => invoke<string>("apply_hotkeys");
export const setHudEnabled = (enabled: boolean) => invoke<void>("set_hud_enabled", { enabled });

/** The readings of the machine, once a second while the program runs. */
export interface Gpu {
  name: string;
  usage: number | null;
  temp: number | null;
  clockMhz: number | null;
  memUsedMb: number | null;
  memTotalMb: number | null;
}

export interface Process {
  pid: number;
  name: string;
  cpu: number;
  ramMb: number;
}

export interface Sensor {
  chip: string;
  label: string;
  value: number;
}

export interface Battery {
  percent: number;
  /** On the mains: charging, full or held at a limit. */
  plugged: boolean;
}

export interface Snapshot {
  battery: Battery | null;
  cpu: number;
  cores: number[];
  cpuName: string;
  cpuCores: number;
  cpuThreads: number;
  cpuMhz: number | null;
  governor: string | null;
  ramUsedMb: number;
  ramTotalMb: number;
  ramAvailableMb: number;
  swapUsedMb: number;
  swapTotalMb: number;
  cpuTemp: number | null;
  cpuWatts: number | null;
  temps: Sensor[];
  fans: Sensor[];
  voltages: Sensor[];
  powers: Sensor[];
  gpu: Gpu | null;
  netRxKbps: number;
  netTxKbps: number;
  diskReadKbps: number;
  diskWriteKbps: number;
  fps: number | null;
  processes: Process[];
}

export const monitorSnapshot = () => invoke<Snapshot | null>("monitor_snapshot");

/** One knob of the kernel the governor can turn, as the daemon sees it. */
export interface GovernorLever {
  key: string;
  /** "cpu", "memory", "kernel" or "disk". */
  group: string;
  path: string;
  /** What the node is worth right now. */
  value: string;
  /** What it is worth while a game runs. */
  gameValue: string;
  available: boolean;
}

export interface GovernorStatus {
  /** A profile is applied and its original values are saved. */
  active: boolean;
  /** Seconds since the profile was applied, or null. */
  since: number | null;
  /** Which profile is on, when one is. */
  profile: string | null;
  /** The profiles the daemon knows, in the order to show them. */
  profiles: string[];
  original: { key: string; path: string; value: string }[] | null;
  levers: GovernorLever[];
  /** Levers this kernel or this machine does not have, and how to get them. */
  missing: GovernorMissing[];
  /** What the daemon does by itself when a game starts. */
  auto: GovernorAuto;
  /** Games running right now, found through their desktop entries. */
  games: { pid: number; name: string }[];
  /** The game being played, when the automatic profile is on. */
  session: GovernorSession | null;
  /** The last games that have been played. */
  history: GovernorPastSession[];
  /** Total time per game, longest first. */
  totals: { name: string; seconds: number }[];
  /** What each profile would change: profile id -> lever key -> value. */
  preview: Record<string, Record<string, string>>;
}

export interface GovernorAuto {
  enabled: boolean;
  profile: string;
  exclusions: string[];
  /** Games that ask for their own profile, and their scheduler if they want one. */
  rules: { name: string; profile: string; scheduler?: string | null; mode?: number | null }[];
  /** Programs added by hand as games, for those with no desktop entry. */
  manual: string[];
}

/** The game being played, while the automatic profile is on. */
export interface GovernorSession {
  name: string;
  pid: number;
  /** When it started, in seconds since 1970. */
  seconds: number;
  profile: string;
}

/** A game that has already been played to the end. */
export interface GovernorPastSession {
  name: string;
  profile: string;
  seconds: number;
  /** When it closed, in seconds since the epoch. */
  at: number;
}

/** How a lever that is not here could be brought in, if at all. */
export type GovernorFix =
  | { kind: "package"; apt: string | null; flatpak: string | null }
  | { kind: "kernel" }
  | { kind: "hardware" };

export interface GovernorMissing {
  key: string;
  group: string;
  /** Name of the translated string that says why it is missing. */
  reason: string;
  fix: GovernorFix;
}

export const governorStatus = () => invoke<GovernorStatus>("governor_status");

/** A kernel on disk, and what the game mode can use of it. */
export interface KernelInfo {
  version: string;
  active: boolean;
  schedExt: boolean | null;
  btf: boolean | null;
}

export interface KernelsState {
  running: string | null;
  kernels: KernelInfo[];
}

export const kernelsStatus = () => invoke<KernelsState>("kernels_status");

/** The schedulers of sched_ext, as the kernel and their service have them. */
export interface ScxState {
  kernel: boolean;
  service: boolean;
  /** `unknown` while no scheduler is running. */
  current: string;
  /** 0 auto, 1 gaming, 2 powersave, 3 lowlatency, 4 server. */
  mode: number;
  supported: string[];
}

export const scxStatus = () => invoke<ScxState>("scx_status");
export const scxSet = (scheduler: string | null, mode: number) =>
  invoke<void>("scx_set", { scheduler, mode });/** The levers as this machine has them, read without the service. */
export const governorCatalogue = () =>
  invoke<Pick<GovernorStatus, "levers" | "missing" | "totals" | "preview">>("governor_catalogue");
export const governorApply = (profile: string) => invoke<unknown>("governor_apply", { profile });
export const governorRestore = () => invoke<unknown>("governor_restore");
/** Automatic profile: on or off, and which one. */
export const governorAuto = (enabled: boolean, profile: string) =>
  invoke<unknown>("governor_auto", { enabled, profile });
/** Removes a name from the list of what is never taken for a game, or adds it. */
export const governorExclusion = (name: string, exclude: boolean) =>
  invoke<unknown>("governor_exclusion", { name, exclude });
/** Gives one game its own profile and, if it wants one, its scheduler; an empty profile removes the rule. */
export const governorRule = (
  name: string,
  profile: string,
  scheduler: string | null = null,
  mode: number | null = null,
) => invoke<unknown>("governor_rule", { name, profile, scheduler, mode });
/** Adds a program by hand to what counts as a game, or removes it. */
export const governorManual = (name: string, add: boolean) =>
  invoke<unknown>("governor_manual", { name, add });
/** Installs a package through pkexec; polkit keeps the password for a while. */
export const installPackages = (apt: string[], flatpak: string[]) =>
  invoke<void>("install_packages", { apt, flatpak });

export interface ServiceState {
  /** The unit of the package is on disk; the .deb puts it there. */
  unitInstalled: boolean;
  running: boolean;
}

/** The daemon of the distribution, which reniced the game until now. */
export interface GameModeState {
  installed: boolean;
  active: boolean;
}

export const governorService = () => invoke<ServiceState>("governor_service");
export const governorServiceControl = (start: boolean) => invoke<void>("governor_service_control", { start });
export const gamemodeState = () => invoke<GameModeState>("gamemode_state");

/** One of the gaming tweaks of Soplos Welcome, and whether it is in place. */
export interface TweakState {
  key: string;
  on: boolean;
  available: boolean;
}

export const tweaksStatus = () => invoke<TweakState[]>("tweaks_status");
export const tweakSet = (which: string, on: boolean) => invoke<void>("tweak_set", { which, on });
export const tweaksRevertAll = () => invoke<void>("tweaks_revert_all");
export const tweaksApplyGaming = () => invoke<void>("tweaks_apply_gaming");
export const overlayToolsInstalled = () => invoke<boolean>("overlay_tools_installed");
export const installOverlayTools = () => invoke<void>("install_overlay_tools");
export const ryzenadjInstalled = () => invoke<boolean>("ryzenadj_installed");
/** The sensor chips of this machine that are not loaded yet. */
export const sensorModules = () => invoke<string[]>("sensor_modules");
/** Asks the kernel for them, with the one permission of the package. */
export const askSensorModules = () => invoke<void>("ask_sensor_modules");

/** The MangoHud configuration of this user, with its comments kept. */
/** One key of MangoHud.conf the window offers as a box, a number or a field. */
export interface MangoHudSetting {
  key: string;
  group: "metrics" | "look" | "limits" | "keys";
  kind: "flag" | "number" | "text";
  min: number | null;
  max: number | null;
  /** The words the file takes, when it takes one of a few. */
  choices: string[];
  /** What a ticked box writes: empty for an option that is only on or off. */
  default: string;
}

export interface MangoHudState {
  path: string;
  exists: boolean;
  /** The keys it already has, in the order of the file. */
  entries: [string, string][];
  /** Comments and blank lines of the file, which are never touched. */
  otherLines: number;
  preset: string | null;
  /** The keys that can be offered, in the order of their groups. */
  settings: MangoHudSetting[];
  /** The folder MangoHud writes its logs to, when the file names one. */
  fpsLog: string | null;
}

export const mangohudStatus = () => invoke<MangoHudState>("mangohud_status");
export const mangohudSet = (changes: [string, string][]) => invoke<MangoHudState>("mangohud_set", { changes });
export const mangohudRemove = (key: string) => invoke<MangoHudState>("mangohud_remove", { key });
export const mangohudPreset = (name: string) => invoke<MangoHudState>("mangohud_preset", { name });
export const mangohudText = () => invoke<{ text: string }>("mangohud_text");
/** Points MangoHud at a log the panel can read, which is where the FPS come from. */
export const mangohudFpsLog = () => invoke<MangoHudState>("mangohud_fps_log");
export const mangohudFpsLogOff = () => invoke<MangoHudState>("mangohud_fps_log_off");

/** One game installed, as the window shows it. */
export interface GameEntry {
  id: string;
  name: string;
  /** "menu" when it comes from the desktop, "steam" when from its manifests,
   * "lutris" when Lutris lists it. */
  source: "menu" | "steam" | "lutris";
  /** The program the daemon looks for in /proc, when the entry names one. */
  process: string | null;
  appId: string | null;
  /** The picture of the desktop, already prepared for the window. */
  icon: string | null;
  /** The cover: the one the user chose, else the one Steam keeps. */
  cover: string | null;
}

export const libraryList = () => invoke<GameEntry[]>("library_list");
export const libraryLaunch = (id: string) => invoke<void>("library_launch", { id });
export const librarySetCover = (id: string, path: string) => invoke<void>("library_set_cover", { id, path });
export const libraryClearCover = (id: string) => invoke<void>("library_clear_cover", { id });

/** One setting of Feral GameMode, and the value it has in gamemode.ini. */
export interface GameModeSetting {
  section: string;
  key: string;
  /** null when the file does not carry the key yet. */
  value: string | null;
  default: string;
  choices: string[];
}

export interface GameModeIni {
  path: string;
  exists: boolean;
  /** A file of the system exists too; the panel never touches it. */
  system: boolean;
  settings: GameModeSetting[];
  keys: [string, string][];
}

export const gamemodeIniStatus = () => invoke<GameModeIni>("gamemode_ini_status");
export const gamemodeIniSet = (section: string, key: string, value: string) =>
  invoke<GameModeIni>("gamemode_ini_set", { section, key, value });
export const gamemodeIniRemove = (section: string, key: string) =>
  invoke<GameModeIni>("gamemode_ini_remove", { section, key });
