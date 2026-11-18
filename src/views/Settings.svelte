<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { _ } from "svelte-i18n";
  import { Check, AlertTriangle } from "lucide-svelte";
  import { getSettings, setSettings, applyHotkeys, type Settings, type HudCorner, type HudMode, type HudSettings, type HudMetric } from "../lib/api";
  import Switch from "../components/Switch.svelte";

  let s: Settings | null = null;
  let note: { ok: boolean; text: string } | null = null;

  const corners: HudCorner[] = ["top-left", "top-right", "bottom-left", "bottom-right"];
  const sides: { key: "left" | "right"; label: string }[] = [
    { key: "left", label: "settings.hud.sides.left" },
    { key: "right", label: "settings.hud.sides.right" },
  ];
  const modes: { key: HudMode; label: string; text: string }[] = [
    { key: "simple", label: "settings.hud.modes.simple", text: "settings.hud.modes.simpleText" },
    { key: "advanced", label: "settings.hud.modes.advanced", text: "settings.hud.modes.advancedText" },
  ];
  const metrics: HudMetric[] = [
    "cpu",
    "cpuTemp",
    "cpuWatts",
    "clocks",
    "gpu",
    "gpuTemp",
    "vram",
    "ram",
    "fps",
    "frametime",
    "net",
  ];

  let unlistenHud: (() => void) | undefined;

  onMount(async () => {
    try {
      s = await getSettings();
    } catch (e) {
      note = { ok: false, text: String(e) };
    }
    try {
      // The tray and the shortcut turn the overlay on or off from outside.
      unlistenHud = await listen<boolean>("hud-enabled", (event) => {
        if (s) s.hud.enabled = event.payload;
      });
    } catch {
      // Not running inside the program: nothing to listen to.
    }
  });

  onDestroy(() => unlistenHud?.());

  async function patch(change: Partial<Settings>) {
    if (!s) return;
    try {
      s = await setSettings(change);
      note = null;
    } catch (e) {
      note = { ok: false, text: String(e) };
    }
  }

  async function patchHud(change: Partial<HudSettings>) {
    if (!s) return;
    await patch({ hud: { ...s.hud, ...change } });
  }

  function chooseCorner(corner: HudCorner) {
    void patchHud({ corner });
  }

  // The tall panel has neither top nor bottom: it only leans on a side.
  function chooseSide(side: "left" | "right") {
    void patchHud({ corner: side === "left" ? "top-left" : "top-right" });
  }

  function onMode(mode: HudMode) {
    void patchHud({ mode });
  }

  // The number follows the slider while it is dragged, but the change is only
  // written when it is let go: each write asks the program to move the overlay.
  function slide(key: "opacity" | "scale", event: Event) {
    if (!s) return;
    s.hud[key] = Number((event.currentTarget as HTMLInputElement).value);
    s = s;
  }

  function commitSlide(key: "opacity" | "scale") {
    if (!s) return;
    void patchHud(key === "opacity" ? { opacity: s.hud.opacity } : { scale: s.hud.scale });
  }

  function onMetric(key: HudMetric, event: Event) {
    if (!s) return;
    const current = s.hud.metrics;
    const checked = (event.currentTarget as HTMLInputElement).checked;
    // The list keeps the order it was designed in, whichever order the boxes
    // are ticked in.
    const next = metrics.filter((one) => (one === key ? checked : current.includes(one)));
    void patchHud({ metrics: next });
  }

  function metricOn(key: HudMetric): boolean {
    return s ? s.hud.metrics.includes(key) : false;
  }

  // The shortcut is pressed, not typed. The field only says which of the two
  // is listening; the keys arrive at the window, so a click that never reaches
  // the field cannot leave the box deaf.
  let capturing: "panel" | "hud" | null = null;
  const held = ["Control", "Shift", "Alt", "Meta"];
  const namedKeys: Record<string, string> = {
    " ": "Space",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    "+": "Plus",
    "-": "Minus",
  };

  async function captureKey(event: KeyboardEvent) {
    if (!capturing || !s) return;
    // The window listens for Ctrl+Q and Ctrl+Tab: here the keys are being
    // written down, not used.
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      capturing = null;
      return;
    }
    if (held.includes(event.key)) return;
    const parts: string[] = [];
    if (event.ctrlKey) parts.push("Ctrl");
    if (event.altKey) parts.push("Alt");
    if (event.shiftKey) parts.push("Shift");
    if (event.metaKey) parts.push("Super");
    if (parts.length === 0) return;
    event.preventDefault();
    const key = namedKeys[event.key] ?? (event.key.length === 1 ? event.key.toUpperCase() : event.key);
    const combo = [...parts, key].join("+");
    const which = capturing;
    capturing = null;
    if (which === "panel") s.panelHotkey = combo;
    else s.hudHotkey = combo;
    s = s;
    await patch({ panelHotkey: s.panelHotkey, hudHotkey: s.hudHotkey });
    const refused = await applyHotkeys();
    note = refused ? { ok: false, text: refused } : null;
  }
</script>

<svelte:window on:keydown|capture={captureKey} />

{#if !s}
  <p class="text-[0.85rem] text-text-muted">{$_("common.loading")}</p>
{:else}
  <section class="mb-6">
    <h2 class="text-[0.95rem] font-semibold text-text-bright mb-1">{$_("settings.hotkeys.title")}</h2>
    <p class="text-[0.78rem] text-text-muted mb-3">{$_("settings.hotkeys.hint")}</p>
    <div class="flex flex-col gap-3 sm:flex-row sm:flex-wrap sm:items-end">
      <label class="flex flex-col gap-1.5 sm:w-[200px] sm:shrink-0">
        <span class="text-[0.78rem] text-text">{$_("settings.hotkeys.panel")}</span>
        <input
          readonly
          class="h-8 border rounded px-2.5 text-[0.82rem] text-text-bright cursor-pointer outline-none transition-colors {capturing === 'panel'
            ? 'border-primary bg-primary/10 ring-2 ring-primary/50'
            : 'border-border bg-surface-2'}"
          value={s.panelHotkey}
          on:click={() => (capturing = "panel")}
          on:blur={() => (capturing = null)}
        />
      </label>
      <label class="flex flex-col gap-1.5 sm:w-[200px] sm:shrink-0">
        <span class="text-[0.78rem] text-text">{$_("settings.hotkeys.hud")}</span>
        <input
          readonly
          class="h-8 border rounded px-2.5 text-[0.82rem] text-text-bright cursor-pointer outline-none transition-colors {capturing === 'hud'
            ? 'border-primary bg-primary/10 ring-2 ring-primary/50'
            : 'border-border bg-surface-2'}"
          value={s.hudHotkey}
          on:click={() => (capturing = "hud")}
          on:blur={() => (capturing = null)}
        />
      </label>
      <div class="flex flex-col gap-1.5 sm:w-[180px] sm:shrink-0">
        <span class="text-[0.78rem] text-text whitespace-nowrap">{$_("settings.session.autostart")}</span>
        <div class="h-8 flex items-center">
          <Switch checked={s.startWithSession} label={$_("settings.session.autostart")} on:change={(e) => patch({ startWithSession: e.detail })} />
        </div>
      </div>
    </div>
  </section>

  <section class="mb-6">
    <h2 class="text-[0.95rem] font-semibold text-text-bright mb-1">{$_("settings.hud.title")}</h2>
    <p class="text-[0.78rem] text-text-muted mb-3">{$_("settings.hud.hint")}</p>

    <div class="border border-border rounded-lg divide-y divide-border">
      <div class="flex items-center gap-3 px-3 py-2.5">
        <Switch checked={s.hud.enabled} label={$_("settings.hud.enabled")} on:change={(e) => patchHud({ enabled: e.detail })} />
        <span class="text-[0.82rem] text-text">{$_("settings.hud.enabled")}</span>
      </div>

      <div class="px-3 py-3">
        <p class="text-[0.72rem] uppercase tracking-wide text-text-muted mb-2">{$_("settings.hud.mode")}</p>
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
          {#each modes as mode (mode.key)}
            <button
              class="text-left rounded border px-3 py-2 transition-colors {s.hud.mode === mode.key
                ? 'border-primary bg-primary/10'
                : 'border-border hover:bg-surface-2'}"
              on:click={() => onMode(mode.key)}
            >
              <p class="text-[0.84rem] text-text-bright">{$_(mode.label)}</p>
              <p class="text-[0.72rem] text-text-muted mt-0.5 leading-snug">{$_(mode.text)}</p>
            </button>
          {/each}
        </div>
      </div>

      <div class="px-3 py-3 flex flex-col gap-4 sm:flex-row sm:items-start sm:gap-6">
        <!-- Where the overlay sits is a corner, and a corner is picked on a
             small screen instead of in a list of words. -->
        <div class="flex flex-col gap-1.5 shrink-0">
          <span class="text-[0.74rem] text-text-muted">{$_("settings.hud.corner")}</span>
          <!-- The small card takes a corner; the tall panel only leans on a
               side, so each shape is picked where it really lands. -->
          <div class="grid w-[124px] aspect-[16/10] p-1 gap-1 rounded border border-border bg-dark {s.hud.mode === "simple" ? 'grid-cols-2 grid-rows-2' : 'grid-cols-2 grid-rows-1'}">
            {#if s.hud.mode === "simple"}
              {#each corners as corner, i (corner)}
                <button
                  type="button"
                  class="flex items-start rounded-sm transition-colors {s.hud.corner === corner ? 'bg-primary/15' : 'hover:bg-surface-2'}"
                  class:justify-end={i % 2 === 1}
                  class:items-end={i > 1}
                  aria-label={$_(`settings.hud.corners.${corner.replace("-", "")}`)}
                  title={$_(`settings.hud.corners.${corner.replace("-", "")}`)}
                  on:click={() => chooseCorner(corner)}
                >
                  <span class="w-4 h-2.5 rounded-[1px] {s.hud.corner === corner ? 'bg-primary' : 'bg-text-muted/40'}" />
                </button>
              {/each}
            {:else}
              {#each sides as side (side.key)}
                <button
                  type="button"
                  class="flex rounded-sm transition-colors {s.hud.corner.endsWith(side.key) ? 'bg-primary/15' : 'hover:bg-surface-2'}"
                  class:justify-end={side.key === "right"}
                  aria-label={$_(side.label)}
                  title={$_(side.label)}
                  on:click={() => chooseSide(side.key)}
                >
                  <span class="w-2.5 h-full rounded-[1px] {s.hud.corner.endsWith(side.key) ? 'bg-primary' : 'bg-text-muted/40'}" />
                </button>
              {/each}
            {/if}
          </div>
        </div>

        <!-- Opacity and size are a position on a scale, so they are read and
             moved at the same time, without a box to type in. -->
        <div class="flex flex-col gap-3 flex-1 min-w-0">
          <label class="flex flex-col gap-1">
            <span class="flex items-baseline justify-between gap-2">
              <span class="text-[0.74rem] text-text-muted">{$_("settings.hud.opacity")}</span>
              <span class="text-[0.78rem] text-text-bright tabular-nums">{s.hud.opacity}</span>
            </span>
            <input
              type="range"
              min="20"
              max="100"
              step="5"
              value={s.hud.opacity}
              class="w-full accent-[#ff7d2e] cursor-pointer"
              on:input={(event) => slide("opacity", event)}
              on:change={() => commitSlide("opacity")}
            />
          </label>
          <label class="flex flex-col gap-1">
            <span class="flex items-baseline justify-between gap-2">
              <span class="text-[0.74rem] text-text-muted">{$_("settings.hud.scale")}</span>
              <span class="text-[0.78rem] text-text-bright tabular-nums">{s.hud.scale}</span>
            </span>
            <input
              type="range"
              min="50"
              max="200"
              step="5"
              value={s.hud.scale}
              class="w-full accent-[#ff7d2e] cursor-pointer"
              on:input={(event) => slide("scale", event)}
              on:change={() => commitSlide("scale")}
            />
          </label>
        </div>
      </div>

      <div class="px-3 py-3">
        <p class="text-[0.72rem] uppercase tracking-wide text-text-muted mb-2">{$_("settings.hud.metricsTitle")}</p>
        <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-2">
          {#each metrics as metric (metric)}
            <label class="flex items-center gap-2 text-[0.8rem] text-text">
              <input type="checkbox" class="accent-[#ff7d2e]" checked={metricOn(metric)} on:change={(e) => onMetric(metric, e)} />
              {$_(`settings.hud.metrics.${metric}`)}
            </label>
          {/each}
        </div>
      </div>
    </div>
  </section>

  {#if note}
    <p class="flex items-center gap-1.5 text-[0.78rem] {note.ok ? 'text-success' : 'text-danger'}">
      {#if note.ok}<Check size={14} />{:else}<AlertTriangle size={14} />{/if}
      {note.text}
    </p>
  {/if}
{/if}
