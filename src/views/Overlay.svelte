<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { _ } from "svelte-i18n";
  import { Activity, ArrowDownUp, Cpu, Gauge, HardDrive, MemoryStick, Monitor, Scale, SlidersHorizontal, Snowflake, Thermometer, Timer, Zap } from "lucide-svelte";
  import {
    getSettings,
    governorApply,
    governorAuto,
    governorRestore,
    governorServiceControl,
    governorService,
    governorStatus,
    monitorSnapshot,
    tweakSet,
    tweaksStatus,
    type Gpu,
    type GovernorStatus,
    type HudSettings,
    type ServiceState,
    type Snapshot,
    type TweakState,
  } from "../lib/api";

  // The surface over the game: what it says is what a player looks at while
  // playing, so it holds few numbers and no settings.
  let snapshot: Snapshot | null = null;
  let status: GovernorStatus | null = null;
  let service: ServiceState | null = null;
  let tweaks: TweakState[] = [];
  let hud: HudSettings | null = null;
  let busy = false;
  let timer: ReturnType<typeof setInterval> | undefined;

  const tweakIcons: Record<string, typeof Zap> = {
    sysctl: SlidersHorizontal,
    gpu: Monitor,
    schedulers: HardDrive,
    performance: Gauge,
    prime_run: Cpu,
  };

  const profileIcons: Record<string, typeof Zap> = {
    max: Zap,
    balanced: Scale,
    quiet: Snowflake,
  };

  $: active = status?.active ?? false;
  $: profile = status?.profile ?? null;
  $: game = status?.session?.name ?? null;
  $: gpu = snapshot?.gpu ?? null;

  /** The card is painted with the transparency the settings ask for. */
  $: background = `rgb(var(--color-dark) / ${(hud?.opacity ?? 100) / 100})`;

  /** Everything drawn inside grows or shrinks with the scale. The window is
   * resized to the same proportion by the backend, so the numbers and the
   * frame grow together and nothing is cut. */
  $: zoom = (hud?.scale ?? 100) / 100;

  /** As many columns as cells are shown, up to three. */
  $: columns = `repeat(${Math.max(1, Math.min(cells.length, 3))}, minmax(0, 1fr))`;

  type Cell = {
    key: string;
    value: string;
    /** A second figure under the first one, when one reading is not enough. */
    sub: string | null;
    bar: number | null;
    icon: typeof Zap;
    tint: string;
  };

  const tints: Record<string, string> = {
    cpu: "bg-primary/20 text-primary",
    cpuTemp: "bg-warning/20 text-warning",
    cpuWatts: "bg-warning/20 text-warning",
    clocks: "bg-secondary/20 text-secondary",
    gpu: "bg-accent/20 text-accent",
    gpuTemp: "bg-warning/20 text-warning",
    vram: "bg-accent/20 text-accent",
    ram: "bg-secondary/20 text-secondary",
    fps: "bg-success/20 text-success",
    frametime: "bg-success/20 text-success",
    net: "bg-primary/20 text-primary",
  };

  const icons: Record<string, typeof Zap> = {
    cpu: Cpu,
    cpuTemp: Thermometer,
    cpuWatts: Zap,
    clocks: Gauge,
    gpu: Monitor,
    gpuTemp: Thermometer,
    vram: Monitor,
    ram: MemoryStick,
    fps: Activity,
    frametime: Timer,
    net: ArrowDownUp,
  };

  /** One card per reading the user asked for. The readings come in as
   * arguments so that the card is redrawn with every tick. */
  function cellOf(key: string, snap: Snapshot | null, card: Gpu | null): Cell | null {
    const icon = icons[key];
    if (!icon) return null;
    const cell = (value: string, sub: string | null = null, bar: number | null = null): Cell => ({
      key,
      value,
      sub,
      bar,
      icon,
      tint: tints[key],
    });
    switch (key) {
      case "cpu":
        return cell(percent(snap?.cpu), null, snap ? Math.min(100, Math.round(snap.cpu)) : null);
      case "cpuTemp":
        return cell(temperature(snap?.cpuTemp));
      case "cpuWatts":
        return cell(watts(snap?.cpuWatts));
      case "clocks":
        return cell(mhz(snap?.cpuMhz), card?.clockMhz == null ? null : `GPU ${mhz(card.clockMhz)}`);
      case "gpu":
        return cell(percent(card?.usage), null, card?.usage == null ? null : Math.min(100, Math.round(card.usage)));
      case "gpuTemp":
        return cell(temperature(card?.temp));
      case "vram":
        return card?.memTotalMb
          ? cell(`${card.memUsedMb ?? 0} MB`, `${card.memTotalMb} MB`)
          : cell("—");
      case "ram":
        return snap
          ? cell(memory(snap.ramUsedMb, snap.ramTotalMb), `${snap.ramUsedMb} MB`, memoryBar(snap.ramUsedMb, snap.ramTotalMb))
          : cell("—");
      case "fps":
        return cell(frames(snap?.fps));
      case "frametime":
        return cell(frametime(snap?.fps));
      case "net":
        return cell(`↓ ${speed(snap?.netRxKbps)}`, `↑ ${speed(snap?.netTxKbps)}`);
      default:
        return null;
    }
  }

  $: cells = (hud?.metrics ?? [])
    .map((key) => cellOf(key, snapshot, gpu))
    .filter((cell): cell is Cell => cell != null);

  function memoryBar(used: number, total: number): number | null {
    if (total <= 0) return null;
    return Math.min(100, Math.round((used / total) * 100));
  }

  function percent(value: number | null | undefined): string {
    return value == null ? "—" : `${Math.round(value)} %`;
  }

  function memory(used: number, total: number): string {
    if (total <= 0) return "—";
    return `${Math.round((used / total) * 100)} %`;
  }

  function temperature(value: number | null | undefined): string {
    return value == null ? "—" : `${Math.round(value)} °C`;
  }

  function frames(value: number | null | undefined): string {
    return value == null ? "—" : String(Math.round(value));
  }

  function watts(value: number | null | undefined): string {
    return value == null ? "—" : `${Math.round(value)} W`;
  }

  function mhz(value: number | null | undefined): string {
    return value == null ? "—" : `${Math.round(value)} MHz`;
  }

  function frametime(fps: number | null | undefined): string {
    return fps == null || fps <= 0 ? "—" : `${(1000 / fps).toFixed(1)} ms`;
  }

  /** Kilobytes per second, as the size a player reads: MB when there are. */
  function speed(kbps: number | null | undefined): string {
    if (kbps == null) return "—";
    return kbps >= 1024 ? `${(kbps / 1024).toFixed(1)} MB/s` : `${Math.round(kbps)} KB/s`;
  }

  /** The readings of the machine, once a second. */
  async function readMachine() {
    try {
      snapshot = await monitorSnapshot();
    } catch {
      snapshot = null;
    }
  }

  // The rest changes only when somebody does something: it is read every few
  // seconds, and at once after a tap of the overlay. Asking the daemon for its
  // state makes it look at every game entry and every process, which is not
  // something to do once a second while a game runs.
  let tick = 0;
  async function everySecond() {
    tick += 1;
    if (tick % 5 === 0) await refresh();
    else await readMachine();
  }

  async function refresh() {
    await readMachine();
    try {
      status = await governorStatus();
    } catch {
      status = null;
    }
    try {
      hud = (await getSettings()).hud;
    } catch {
      // Without the file the overlay keeps the last settings it read.
    }
    try {
      tweaks = await tweaksStatus();
    } catch {
      tweaks = [];
    }
    try {
      service = await governorService();
    } catch {
      service = null;
    }
  }

  /** The same three profiles as the window, one tap each. */
  async function choose(profile: string) {
    if (busy) return;
    busy = true;
    try {
      if (service && !service.running) await governorServiceControl(true);
      await governorApply(profile);
      if (status?.auto?.enabled) await governorAuto(true, profile);
    } catch {
      // The next reading shows what stayed.
    }
    await refresh();
    busy = false;
  }

  async function flipTweak(key: string, on: boolean) {
    if (busy) return;
    busy = true;
    try {
      await tweakSet(key, on);
    } catch {
      // The next reading shows what stayed.
    }
    await refresh();
    busy = false;
  }

  async function toggleMode() {
    if (busy) return;
    busy = true;
    try {
      if (active) {
        await governorRestore();
      } else {
        await governorApply(profile ?? status?.auto?.profile ?? "max");
      }
    } catch {
      // The next reading shows the state that stayed.
    }
    await refresh();
    busy = false;
  }

  onMount(() => {
    refresh();
    timer = setInterval(everySecond, 1000);
  });

  // The root font size is what the whole card is measured against, so raising
  // it is what makes the overlay bigger without redrawing it differently.
  $: if (typeof document !== "undefined") {
    document.documentElement.style.fontSize = `${16 * zoom}px`;
  }

  onDestroy(() => clearInterval(timer));
</script>

{#if (hud?.mode ?? "simple") === "advanced"}
<div
  class="w-screen h-screen flex flex-col overflow-hidden border-l border-border"
  style="background-color: {background};"
>
  <div class="flex items-center gap-2.5 px-4 py-3 border-b border-border shrink-0" data-tauri-drag-region>
    <Zap size={17} class={active ? "text-primary" : "text-text-muted"} />
    <span class="text-[0.92rem] text-text-bright truncate flex-1">Soplos Gaming Panel</span>
  </div>

  <div class="flex-1 overflow-y-auto px-4 py-4 flex flex-col gap-5">
    <section
      class="relative shrink-0 overflow-hidden rounded-xl border p-4 {active ? 'border-primary/60' : 'border-border'}"
      style="background: linear-gradient(160deg, rgb(var(--color-primary) / {active ? 0.2 : 0.07}), transparent 65%);"
    >
      <div class="flex items-center gap-3.5">
        <div class="relative shrink-0">
          <svg viewBox="0 0 44 44" class="w-[58px] h-[58px] -rotate-90">
            <circle cx="22" cy="22" r="18" fill="none" stroke="rgb(var(--color-surface-2))" stroke-width="3.5" />
            <circle
              cx="22"
              cy="22"
              r="18"
              fill="none"
              stroke="rgb(var(--color-primary))"
              stroke-width="3.5"
              stroke-linecap="round"
              stroke-dasharray="113.1"
              stroke-dashoffset={active ? 0 : 113.1}
              class="transition-all duration-500"
            />
          </svg>
          <span class="absolute inset-0 flex items-center justify-center">
            <Zap size={20} class={active ? "text-primary" : "text-text-muted"} />
          </span>
        </div>
        <div class="min-w-0 flex-1">
          <p class="text-[1.12rem] font-semibold text-text-bright leading-tight">
            {active ? $_("gamemode.ready") : $_("gamemode.idle")}
          </p>
          <p class="text-[0.78rem] text-text-muted mt-0.5 truncate">
            {game ? $_("gamemode.playing", { values: { name: game } }) : $_("gamemode.noGames")}
          </p>
        </div>
      </div>
      <button
        class="w-full mt-3.5 py-2.5 rounded-lg text-[0.9rem] font-medium transition-colors disabled:opacity-40 {active
          ? 'border border-border text-text-bright hover:bg-surface-2'
          : 'bg-primary text-white hover:opacity-90'}"
        disabled={busy}
        on:click={toggleMode}
      >
        {active ? $_("gamemode.turnOff") : $_("gamemode.turnOn")}
      </button>
    </section>

    {#if cells.length > 0}
      <section class="shrink-0">
        <p class="text-[0.68rem] uppercase tracking-wide text-text-muted mb-2">{$_("monitor.title")}</p>
        <div class="grid grid-cols-2 gap-2">
          {#each cells as cell (cell.key)}
            <div class="rounded-lg border border-border px-2.5 py-2" style="background-color: {background};">
              <div class="flex items-center gap-1.5 min-w-0">
                <span class="w-5 h-5 rounded flex items-center justify-center shrink-0 {cell.tint}">
                  <svelte:component this={cell.icon} size={11} />
                </span>
                <span class="text-[0.62rem] uppercase tracking-wide text-text-muted truncate">{$_(`settings.hud.metrics.${cell.key}`)}</span>
              </div>
              <p class="text-[1.25rem] font-semibold text-text-bright leading-tight mt-1 tabular-nums truncate">
                {cell.value}
              </p>
              {#if cell.sub}
                <p class="text-[0.68rem] text-text-muted leading-tight mt-0.5 tabular-nums truncate">{cell.sub}</p>
              {/if}
              {#if cell.bar != null}
                <div class="h-1 rounded-full bg-surface-2 mt-1.5 overflow-hidden">
                  <div class="h-full rounded-full bg-primary transition-all duration-500" style="width: {cell.bar}%;"></div>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      </section>
    {/if}

  <section class="shrink-0">
    <p class="text-[0.68rem] uppercase tracking-wide text-text-muted mb-2">{$_("gamemode.strength")}</p>
    <div class="grid grid-cols-3 gap-1.5">
      {#each status?.profiles ?? ["max", "balanced", "quiet"] as option (option)}
        <button
          class="flex flex-col items-center gap-1.5 rounded-lg border px-1 py-2.5 transition-colors disabled:opacity-40 {profile === option
            ? 'border-primary bg-primary/10'
            : 'border-border hover:bg-surface-2'}"
          disabled={busy}
          on:click={() => choose(option)}
        >
          <span class="w-7 h-7 rounded-md flex items-center justify-center {profile === option ? 'bg-primary/25 text-primary' : 'bg-surface-2 text-text-muted'}">
            <svelte:component this={profileIcons[option]} size={14} />
          </span>
          <span class="text-[0.66rem] leading-tight text-center {profile === option ? 'text-text-bright' : 'text-text-muted'}">
            {$_(`profile.${option}`)}
          </span>
        </button>
      {/each}
    </div>
  </section>

  <section class="shrink-0">
    <p class="text-[0.68rem] uppercase tracking-wide text-text-muted mb-2">{$_("tweak.title")}</p>
    <div class="grid grid-cols-3 gap-1.5">
      {#each tweaks as tweak (tweak.key)}
        <button
          class="flex flex-col items-center gap-1.5 rounded-lg border px-1 py-2.5 transition-colors disabled:cursor-not-allowed {tweak.on
            ? 'border-primary bg-primary/10'
            : 'border-border hover:bg-surface-2'} {tweak.available ? '' : 'opacity-40'}"
          disabled={busy || !tweak.available}
          on:click={() => flipTweak(tweak.key, !tweak.on)}
          title={tweak.available ? "" : $_("tweak.unavailable")}
        >
          <span class="w-7 h-7 rounded-md flex items-center justify-center {tweak.on ? 'bg-primary/25 text-primary' : 'bg-surface-2 text-text-muted'}">
            <svelte:component this={tweakIcons[tweak.key] ?? Zap} size={14} />
          </span>
          <span class="text-[0.64rem] leading-tight text-center {tweak.on ? 'text-text-bright' : 'text-text-muted'}">
            {$_(`tweak.${tweak.key}`)}
          </span>
        </button>
      {/each}
    </div>
  </section>
  </div>
</div>
{:else}
<div
  class="w-screen h-screen flex flex-col border border-border rounded-lg overflow-hidden"
  style="background-color: {background};"
  data-tauri-drag-region
>
  <div
    class="flex items-center gap-2.5 px-3 py-2 border-b border-border shrink-0"
    style="background: linear-gradient(120deg, rgb(var(--color-primary) / {active ? 0.22 : 0.06}), transparent 70%);"
    data-tauri-drag-region
  >
    <span class="relative w-6 h-6 shrink-0 flex items-center justify-center">
      <span class="absolute inset-0 rounded-full {active ? 'bg-primary/25' : 'bg-surface-2'}"></span>
      <Zap size={13} class={active ? "text-primary relative" : "text-text-muted relative"} />
    </span>
    <span class="text-[0.8rem] text-text-bright truncate flex-1" data-tauri-drag-region>
      {active ? $_("gamemode.ready") : $_("gamemode.idle")}
    </span>
    {#if game}
      <span class="shrink-0 text-[0.66rem] text-text-muted truncate max-w-[40%]" data-tauri-drag-region>{game}</span>
    {/if}
  </div>

  {#if cells.length > 0}
    <div class="grid gap-px bg-border" style="grid-template-columns: {columns};">
      {#each cells as cell (cell.key)}
        <div class="px-2.5 py-2 min-w-0" style="background-color: {background};">
          <div class="flex items-center gap-1 min-w-0">
            <span class="w-4 h-4 rounded flex items-center justify-center shrink-0 {cell.tint}">
              <svelte:component this={cell.icon} size={9} />
            </span>
            <span class="text-[0.58rem] uppercase tracking-wide text-text-muted truncate">{$_(`settings.hud.metrics.${cell.key}`)}</span>
          </div>
          <p class="text-[1.05rem] font-semibold text-text-bright leading-tight mt-0.5 tabular-nums truncate">
            {cell.value}
          </p>
          {#if cell.sub}
            <p class="text-[0.58rem] text-text-muted leading-tight tabular-nums truncate">{cell.sub}</p>
          {/if}
          {#if cell.bar != null}
            <div class="h-[3px] rounded-full bg-surface-2 mt-1 overflow-hidden">
              <div class="h-full rounded-full bg-primary transition-all duration-500" style="width: {cell.bar}%;"></div>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  <div class="flex items-center gap-1.5 px-2 py-1.5 border-t border-border" style="background-color: {background};">
    {#each status?.profiles ?? ["max", "balanced", "quiet"] as option (option)}
      <button
        class="flex-1 min-w-0 flex items-center justify-center gap-1 px-1 py-1.5 rounded border transition-colors disabled:opacity-40 {profile === option
          ? 'border-primary bg-primary/15 text-text-bright'
          : 'border-border text-text-muted hover:text-text-bright'}"
        disabled={busy}
        on:click={() => choose(option)}
        title={$_(`profile.${option}`)}
      >
        <svelte:component this={profileIcons[option]} size={11} class={profile === option ? "text-primary" : ""} />
        <span class="text-[0.6rem] truncate">{$_(`profile.${option}`)}</span>
      </button>
    {/each}
  </div>

  <div class="flex items-center gap-1.5 px-2 py-1.5 border-t border-border" style="background-color: {background};">
    {#each tweaks as tweak (tweak.key)}
      <button
        class="flex-1 min-w-0 h-7 rounded flex items-center justify-center transition-colors disabled:cursor-not-allowed {tweak.on
          ? 'bg-primary/20 text-primary'
          : 'bg-surface-2 text-text-muted hover:text-text-bright'} {tweak.available ? '' : 'opacity-40'}"
        disabled={busy || !tweak.available}
        on:click={() => flipTweak(tweak.key, !tweak.on)}
        title={$_(`tweak.${tweak.key}`)}
        aria-label={$_(`tweak.${tweak.key}`)}
      >
        <svelte:component this={tweakIcons[tweak.key] ?? Zap} size={13} />
      </button>
    {/each}
  </div>

  <div class="flex items-center gap-2 px-3 py-2 border-t border-border" style="background-color: {background};">
    <Gauge size={14} class="text-text-muted shrink-0" />
    <span class="text-[0.72rem] text-text-muted truncate flex-1">
      {game ? $_("gamemode.playing", { values: { name: game } }) : $_("gamemode.noGames")}
    </span>
    <button
      class="shrink-0 px-2.5 py-1 rounded text-[0.74rem] transition-colors disabled:opacity-40 {active
        ? 'border border-border text-text-bright hover:bg-surface-2'
        : 'bg-primary text-white hover:opacity-90'}"
      disabled={busy}
      on:click={toggleMode}
    >
      {active ? $_("gamemode.turnOff") : $_("gamemode.turnOn")}
    </button>
  </div>
</div>
{/if}
