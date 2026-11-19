<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { _ } from "svelte-i18n";
  import { Activity, ArrowDownUp, Cpu, Fan, Gauge, HardDrive, Layers, ListOrdered, MemoryStick, PlugZap, Thermometer, Zap } from "lucide-svelte";
  import { getSettings, monitorSnapshot, setHudEnabled, askSensorModules, sensorModules, type Snapshot } from "../lib/api";
  import { formatTemp, tempUnit, type TempUnit } from "../lib/units";

  // An hour of readings is kept, one per second; the graph shows the window
  // that is picked below it.
  const CAPACITY = 3600;
  const RANGES = [1, 5, 15, 60] as const;
  const RANGE_KEYS: Record<number, string> = {
    1: "monitor.rangeOne",
    5: "monitor.rangeFive",
    15: "monitor.rangeFifteen",
    60: "monitor.rangeHour",
  };
  // The lines of the graph and the color each one is drawn with.
  const COLORS: Record<string, string> = {
    cpu: "#ff7d2e",
    ram: "#60a5fa",
    gpu: "#fec431",
    temp: "#ef4444",
  };

  let snapshot: Snapshot | null = null;
  // Each reading keeps when it was taken: the window the user picks is a span
  // of time, not a number of samples, so a minute and an hour are drawn to
  // the same width with a different amount of history in them.
  type Reading = { t: number; v: number };
  let cpuHistory: Reading[] = [];
  let ramHistory: Reading[] = [];
  let gpuHistory: Reading[] = [];
  let tempHistory: Reading[] = [];
  let minutes = 1;
  let picked: Record<string, boolean> = { cpu: true, ram: true, gpu: true, temp: true };
  let shown: Record<string, Reading[]> = {};
  let hudOn = false;
  let unlisten: (() => void) | undefined;
  let sensorChips: string[] = [];

  $: span = minutes * 60_000;
  $: shown = {
    cpu: sample(within(cpuHistory, span)),
    ram: sample(within(ramHistory, span)),
    gpu: sample(within(gpuHistory, span)),
    temp: sample(within(tempHistory, span)),
  };
  $: series = [
    { key: "cpu", label: $_("monitor.cpu"), color: COLORS.cpu },
    { key: "ram", label: $_("monitor.ram"), color: COLORS.ram },
    ...(hasGpu ? [{ key: "gpu", label: $_("monitor.gpu"), color: COLORS.gpu }] : []),
    ...(hasTemp ? [{ key: "temp", label: `${$_("monitor.temp")} (°C)`, color: COLORS.temp }] : []),
  ];

  /** The top of the box: the highest line on screen, rounded up, from 20 to
   *  100 so a percentage never lies about its own scale. */
  $: ceiling = (() => {
    const values = series
      .filter((entry) => picked[entry.key])
      .flatMap((entry) => shown[entry.key] ?? [])
      .map((reading) => reading.v);
    const top = values.length > 0 ? Math.max(...values) : 100;
    return Math.min(100, Math.max(20, Math.ceil(top / 20) * 20));
  })();

  function remember(history: Reading[], value: number | null): Reading[] {
    const next = [...history, { t: Date.now(), v: value ?? 0 }];
    return next.length > CAPACITY ? next.slice(next.length - CAPACITY) : next;
  }

  /** The readings of the picked window: what happened in the last minute, or
   *  in the last hour, whichever is asked for. */
  function within(history: Reading[], span: number): Reading[] {
    const start = Date.now() - span;
    return history.filter((reading) => reading.t >= start);
  }

  /** Averages the readings into the points the box can actually show. */
  function sample(readings: Reading[], target = 360): Reading[] {
    if (readings.length <= target) return readings;
    const size = readings.length / target;
    const out: Reading[] = [];
    for (let index = 0; index < target; index += 1) {
      const from = Math.floor(index * size);
      const to = Math.max(Math.floor((index + 1) * size), from + 1);
      let sum = 0;
      for (let cursor = from; cursor < to; cursor += 1) sum += readings[cursor].v;
      out.push({ t: readings[Math.min(to, readings.length) - 1].t, v: sum / (to - from) });
    }
    return out;
  }

  function take(next: Snapshot) {
    snapshot = next;
    const ram = next.ramTotalMb > 0 ? (next.ramUsedMb * 100) / next.ramTotalMb : 0;
    cpuHistory = remember(cpuHistory, next.cpu);
    ramHistory = remember(ramHistory, ram);
    gpuHistory = remember(gpuHistory, next.gpu?.usage ?? null);
    tempHistory = remember(tempHistory, next.cpuTemp);
  }

  /** One line of the graph across the whole box: the window the buttons pick
   *  is the width, and what was read before the program started is drawn flat
   *  at the oldest value so the box is never half empty. */
  function line(readings: Reading[]): string {
    if (readings.length < 2) return "";
    const end = Date.now();
    const start = end - span;
    const at = (reading: Reading) =>
      Math.min(Math.max(((reading.t - start) / span) * 100, 0), 100);
    const height = (value: number) =>
      40 - (Math.min(Math.max(value, 0), ceiling) / ceiling) * 40;
    const parts = [`M0 ${height(readings[0].v).toFixed(2)}`];
    for (const reading of readings) {
      parts.push(`L${at(reading).toFixed(2)} ${height(reading.v).toFixed(2)}`);
    }
    return parts.join(" ");
  }

  /** Length of the arc of a ring of radius 16, for a value out of 100. */
  function arc(value: number | null | undefined): string {
    const percent = Math.max(0, Math.min(value ?? 0, 100));
    return `${((percent / 100) * 100.53).toFixed(2)} 100.53`;
  }

  /** A reading as 0-100, for the bars that are not percentages themselves. */
  function fraction(value: number | null | undefined, top = 100): number {
    return Math.max(0, Math.min(((value ?? 0) / top) * 100, 100));
  }

  function percent(value: number | null | undefined): string {
    return value === null || value === undefined ? "—" : `${Math.round(value)} %`;
  }

  /** The filled area under a line, so the graph reads as a shape and not a thread. */
  function area(readings: Reading[]): string {
    const path = line(readings);
    return path ? `${path} L100 40 L0 40 Z` : "";
  }

  /** The unit is an argument so the figures are written again when it changes. */
  function degrees(value: number | null | undefined, unit: TempUnit): string {
    return formatTemp(value, unit);
  }

  function mhz(value: number | null | undefined): string {
    return value === null || value === undefined ? "—" : `${Math.round(value)} MHz`;
  }

  function speed(value: number | null | undefined): string {
    const kbps = value ?? 0;
    return kbps >= 1024 ? `${(kbps / 1024).toFixed(1)} MB/s` : `${Math.round(kbps)} KB/s`;
  }

  async function toggleHud() {
    hudOn = !hudOn;
    try {
      await setHudEnabled(hudOn);
    } catch {
      hudOn = !hudOn;
    }
  }

  onMount(async () => {
    try {
      const first = await monitorSnapshot();
      if (first) take(first);
    } catch {
      // The sampler has not spoken yet; the first event brings the numbers.
    }
    try {
      hudOn = (await getSettings()).hud.enabled;
    } catch {
      // Without the settings the button starts as off.
    }
    try {
      unlisten = await listen<Snapshot>("monitor:tick", (event) => take(event.payload));
    } catch {
      // Not running inside the program.
    }
    try {
      sensorChips = await sensorModules();
    } catch {
      sensorChips = [];
    }
  });

  /** Asks the kernel for the chips that read the sensors, and reads again. */
  async function loadSensors() {
    try {
      await askSensorModules();
      sensorChips = await sensorModules();
      const next = await monitorSnapshot();
      if (next) take(next);
    } catch {
      // The notice stays: the machine did not take the modules.
    }
  }

  onDestroy(() => unlisten?.());

  $: gpu = snapshot?.gpu ?? null;
  // Without a use figure for the card (a virtual machine, for one) the line
  // and its legend would only be a flat line at zero.
  $: hasGpu = gpu?.usage != null;
  // The same goes for a temperature the machine cannot read.
  $: hasTemp = snapshot?.cpuTemp != null;
  $: ramPercent = snapshot && snapshot.ramTotalMb > 0 ? (snapshot.ramUsedMb * 100) / snapshot.ramTotalMb : 0;
  $: hotTemp = (snapshot?.cpuTemp ?? 0) >= 85;
  $: hotCpu = (snapshot?.cpu ?? 0) >= 90;
</script>

<div class="grid grid-cols-4 gap-3.5 mb-3.5">
  <div class="bg-surface border border-border rounded p-3.5 flex items-center gap-3">
    <svg viewBox="0 0 40 40" class="w-[54px] h-[54px] shrink-0 -rotate-90">
      <circle cx="20" cy="20" r="16" fill="none" stroke="rgb(var(--color-surface-2))" stroke-width="4" />
      <circle
        cx="20"
        cy="20"
        r="16"
        fill="none"
        stroke={hotCpu ? "#ef4444" : "#ff7d2e"}
        stroke-width="4"
        stroke-linecap="round"
        stroke-dasharray={arc(snapshot?.cpu)}
      />
    </svg>
    <div class="min-w-0">
      <p class="text-[0.66rem] uppercase tracking-wide text-text-muted flex items-center gap-1.5"><Cpu size={12} />{$_("monitor.cpu")}</p>
      <p class="text-[1.35rem] font-semibold leading-none mt-1 {hotCpu ? 'text-danger' : 'text-text-bright'}">{percent(snapshot?.cpu)}</p>
      <p class="text-[0.7rem] text-text-muted mt-1 truncate">
        {$_("monitor.coresThreads", { values: { cores: snapshot?.cpuCores ?? 0, threads: snapshot?.cpuThreads ?? 0 } })}
      </p>
      <p class="text-[0.7rem] text-text-muted truncate" title={snapshot?.cpuName ?? ""}>
        {mhz(snapshot?.cpuMhz)}{snapshot?.governor ? ` · ${snapshot.governor}` : ""}
      </p>
    </div>
  </div>

  <div class="bg-surface border border-border rounded p-3.5 flex items-center gap-3">
    <svg viewBox="0 0 40 40" class="w-[54px] h-[54px] shrink-0 -rotate-90">
      <circle cx="20" cy="20" r="16" fill="none" stroke="rgb(var(--color-surface-2))" stroke-width="4" />
      <circle
        cx="20"
        cy="20"
        r="16"
        fill="none"
        stroke="#fec431"
        stroke-width="4"
        stroke-linecap="round"
        stroke-dasharray={arc(gpu?.usage)}
      />
    </svg>
    <div class="min-w-0">
      <p class="text-[0.66rem] uppercase tracking-wide text-text-muted flex items-center gap-1.5"><Gauge size={12} />{$_("monitor.gpu")}</p>
      <p class="text-[1.35rem] font-semibold leading-none mt-1 text-text-bright">{percent(gpu?.usage)}</p>
      <p class="text-[0.7rem] text-text-muted mt-1 truncate" title={gpu?.name ?? ""}>
        {gpu ? gpu.name : $_("monitor.noGpu")}
      </p>
      <p class="text-[0.7rem] text-text-muted truncate">
        {gpu?.memUsedMb != null && gpu?.memTotalMb != null
          ? $_("monitor.ofRam", { values: { used: gpu.memUsedMb, total: gpu.memTotalMb } })
          : ""}
      </p>
    </div>
  </div>

  <div class="bg-surface border border-border rounded p-3.5 flex items-center gap-3">
    <svg viewBox="0 0 40 40" class="w-[54px] h-[54px] shrink-0 -rotate-90">
      <circle cx="20" cy="20" r="16" fill="none" stroke="rgb(var(--color-surface-2))" stroke-width="4" />
      <circle cx="20" cy="20" r="16" fill="none" stroke="#60a5fa" stroke-width="4" stroke-linecap="round" stroke-dasharray={arc(ramPercent)} />
    </svg>
    <div class="min-w-0">
      <p class="text-[0.66rem] uppercase tracking-wide text-text-muted flex items-center gap-1.5"><MemoryStick size={12} />{$_("monitor.ram")}</p>
      <p class="text-[1.35rem] font-semibold leading-none mt-1 text-text-bright">{percent(ramPercent)}</p>
      <p class="text-[0.7rem] text-text-muted mt-1 truncate">
        {$_("monitor.ofRam", { values: { used: snapshot?.ramUsedMb ?? 0, total: snapshot?.ramTotalMb ?? 0 } })}
      </p>
      <p class="text-[0.7rem] text-text-muted truncate">
        {$_("monitor.available", { values: { value: snapshot?.ramAvailableMb ?? 0 } })}
      </p>
    </div>
  </div>

  <div class="bg-surface border border-border rounded p-3.5 flex flex-col justify-between">
    <p class="text-[0.66rem] uppercase tracking-wide text-text-muted flex items-center gap-1.5"><Thermometer size={12} />{$_("monitor.temp")}</p>
    <div>
      <p class="text-[1.35rem] font-semibold leading-none {hotTemp ? 'text-danger' : 'text-text-bright'}">{degrees(snapshot?.cpuTemp, $tempUnit)}</p>
      <div class="h-1.5 w-full bg-surface-2 rounded-full mt-2 overflow-hidden">
        <div class="h-full rounded-full {hotTemp ? 'bg-danger' : 'bg-primary'}" style={`width: ${fraction(snapshot?.cpuTemp)}%`}></div>
      </div>
    </div>
    <p class="text-[0.7rem] text-text-muted truncate">
      {gpu?.temp != null ? `${degrees(gpu.temp, $tempUnit)} ${$_("monitor.gpu")}` : $_("monitor.noSensor")}
    </p>
  </div>
</div>

<div class="grid grid-cols-4 gap-3.5 mb-3.5">
  <div class="bg-surface border border-border rounded p-3.5 flex items-start gap-2.5">
    <span class="w-7 h-7 rounded flex items-center justify-center bg-primary/15 shrink-0"><Activity size={14} class="text-primary" /></span>
    <div class="min-w-0">
      <p class="text-[0.66rem] uppercase tracking-wide text-text-muted">{$_("monitor.fps")}</p>
      <p class="text-[1.15rem] font-semibold text-text-bright leading-tight">
        {snapshot?.fps != null ? Math.round(snapshot.fps) : "—"}
      </p>
      <p class="text-[0.7rem] text-text-muted truncate">{snapshot?.fps != null ? "MangoHud" : $_("monitor.noData")}</p>
    </div>
  </div>

  <div class="bg-surface border border-border rounded p-3.5 flex items-start gap-2.5">
    <span class="w-7 h-7 rounded flex items-center justify-center bg-success/15 shrink-0"><ArrowDownUp size={14} class="text-success" /></span>
    <div class="min-w-0">
      <p class="text-[0.66rem] uppercase tracking-wide text-text-muted">{$_("monitor.net")}</p>
      <p class="text-[1.15rem] font-semibold text-text-bright leading-tight">{speed(snapshot?.netRxKbps)}</p>
      <p class="text-[0.7rem] text-text-muted truncate">{$_("monitor.netUp", { values: { value: speed(snapshot?.netTxKbps) } })}</p>
    </div>
  </div>

  <div class="bg-surface border border-border rounded p-3.5 flex items-start gap-2.5">
    <span class="w-7 h-7 rounded flex items-center justify-center bg-warning/15 shrink-0"><HardDrive size={14} class="text-warning" /></span>
    <div class="min-w-0">
      <p class="text-[0.66rem] uppercase tracking-wide text-text-muted">{$_("monitor.disk")}</p>
      <p class="text-[1.15rem] font-semibold text-text-bright leading-tight">{speed(snapshot?.diskReadKbps)}</p>
      <p class="text-[0.7rem] text-text-muted truncate">{$_("monitor.diskWrite", { values: { value: speed(snapshot?.diskWriteKbps) } })}</p>
    </div>
  </div>

  <div class="bg-surface border border-border rounded p-3.5 flex items-start gap-2.5">
    <span class="w-7 h-7 rounded flex items-center justify-center bg-accent/15 shrink-0"><Layers size={14} class="text-accent" /></span>
    <div class="min-w-0 flex-1">
      <p class="text-[0.66rem] uppercase tracking-wide text-text-muted">{$_("monitor.swap")}</p>
      <p class="text-[0.92rem] font-semibold text-text-bright leading-tight break-words">
        {$_("monitor.ofRam", { values: { used: snapshot?.swapUsedMb ?? 0, total: snapshot?.swapTotalMb ?? 0 } })}
      </p>
      <button
        class="mt-1 px-2 py-0.5 rounded border border-border text-[0.68rem] text-text hover:bg-surface-2"
        on:click={toggleHud}
      >
        {hudOn ? $_("monitor.overlayOn") : $_("monitor.overlayOff")}
      </button>
    </div>
  </div>
</div>

<div class="bg-surface border border-border rounded p-4 mb-3.5">
  <div class="flex items-center justify-between mb-2.5">
    <span class="text-[0.66rem] uppercase tracking-wide text-text-muted">{$_("monitor.cores")}</span>
    <span class="text-[0.7rem] text-text-muted">{$_("monitor.cpu")}</span>
  </div>
  <div class="grid gap-1.5" style="grid-template-columns: repeat(auto-fit, minmax(46px, 1fr));">
    {#each (snapshot?.cores ?? []) as core, index}
      <div class="flex flex-col gap-1" title={`${index + 1}: ${Math.round(core)} %`}>
        <div class="h-[46px] w-full bg-surface-2 rounded flex items-end overflow-hidden">
          <div
            class="w-full rounded-t bg-gradient-to-t from-primary/70 to-primary"
            style={`height: ${Math.min(Math.max(core, 2), 100)}%`}
          ></div>
        </div>
        <span class="text-[0.62rem] text-text-muted text-center">{Math.round(core)}</span>
      </div>
    {/each}
  </div>
</div>

<div class="bg-surface border border-border rounded p-4 mb-3.5">
  <div class="flex flex-wrap items-center justify-between gap-2 mb-2">
    <span class="text-[0.78rem] text-text">{$_("monitor.graph")}</span>
    <div class="flex flex-wrap items-center justify-end gap-x-3 gap-y-1.5 text-[0.7rem] text-text-muted">
      {#each series as entry}
        <button
          class="flex items-center gap-1.5 {picked[entry.key] ? '' : 'opacity-40'}"
          title={entry.label}
          on:click={() => (picked[entry.key] = !picked[entry.key])}
        >
          <span class="w-2.5 h-0.5" style={`background: ${entry.color}`}></span>{entry.label}
        </button>
      {/each}
      <span class="flex items-center rounded border border-border overflow-hidden">
        {#each RANGES as range}
          <button class="px-2 py-0.5 {minutes === range ? 'bg-primary/15 text-primary' : ''}" on:click={() => (minutes = range)}>
            {$_(RANGE_KEYS[range])}
          </button>
        {/each}
      </span>
    </div>
  </div>
  <div class="relative">
    <svg viewBox="0 0 100 40" preserveAspectRatio="none" class="w-full h-[150px]">
      {#each [10, 20, 30] as y}
        <line
          x1="0"
          y1={y}
          x2="100"
          y2={y}
          stroke="rgb(var(--color-border))"
          stroke-width="1"
          stroke-opacity="0.7"
          vector-effect="non-scaling-stroke"
        />
      {/each}
      {#each [25, 50, 75] as x}
        <line
          x1={x}
          y1="0"
          x2={x}
          y2="40"
          stroke="rgb(var(--color-border))"
          stroke-width="1"
          stroke-opacity="0.4"
          vector-effect="non-scaling-stroke"
        />
      {/each}
      {#each series as entry}
        {#if picked[entry.key]}
          <path d={area(shown[entry.key])} fill={entry.color} fill-opacity="0.16" stroke="none" />
          <path
            d={line(shown[entry.key])}
            fill="none"
            stroke={entry.color}
            stroke-width="1.8"
            stroke-linejoin="round"
            stroke-linecap="round"
            vector-effect="non-scaling-stroke"
          />
        {/if}
      {/each}
    </svg>
    <!-- The scale of the box, written beside the lines it belongs to. -->
    {#each [25, 50, 75] as percent}
      <span
        class="absolute left-1 text-[0.6rem] text-text-muted tabular-nums pointer-events-none"
        style={`top: ${percent}%; transform: translateY(-50%)`}
      >
        {Math.round(ceiling * (1 - percent / 100))}
      </span>
    {/each}
  </div>
  <!-- The span of time the box covers, so the buttons of the range say what
       they are doing even when there is not yet that much history. -->
  <div class="flex items-center justify-between mt-1 text-[0.6rem] text-text-muted">
    <span>−{$_(RANGE_KEYS[minutes])}</span>
    <span>{$_("monitor.now")}</span>
  </div>
</div>

<div class="grid grid-cols-2 gap-3.5 mb-3.5">
  <div class="bg-surface border border-border rounded p-4">
    <div class="flex items-center gap-1.5 mb-2">
      <Thermometer size={14} class="text-primary" />
      <span class="text-[0.78rem] text-text">{$_("monitor.temperatures")}</span>
    </div>
    {#if (snapshot?.temps ?? []).length > 0}
      <div class="max-h-[190px] overflow-y-auto overscroll-contain inner-scroll">
        <div class="mr-3">
          {#each snapshot?.temps ?? [] as sensor}
            <div class="py-1.5 border-t border-border">
              <div class="flex items-center justify-between text-[0.76rem]">
                <span class="text-text-bright truncate pr-3" title={`${sensor.chip} · ${sensor.label}`}>
                  {sensor.chip} · {sensor.label}
                </span>
                <span class="shrink-0 {sensor.value >= 85 ? 'text-danger' : 'text-text'}">{degrees(sensor.value, $tempUnit)}</span>
              </div>
              <div class="h-1 w-full bg-surface-2 rounded-full mt-1 overflow-hidden">
                <div
                  class="h-full rounded-full {sensor.value >= 85 ? 'bg-danger' : sensor.value >= 70 ? 'bg-warning' : 'bg-primary'}"
                  style={`width: ${fraction(sensor.value)}%`}
                ></div>
              </div>
            </div>
          {/each}
        </div>
      </div>
    {:else}
      <p class="text-[0.76rem] text-text-muted">{$_("monitor.noSensors")}</p>
    {/if}
    {#if sensorChips.length > 0}
      <div class="flex items-center justify-between gap-2 mt-2.5 pt-2 border-t border-border">
        <span class="text-[0.7rem] text-text-muted min-w-0">{$_("monitor.sensorModules")}: {sensorChips.join(", ")}</span>
        <button
          class="shrink-0 px-2 py-0.5 rounded border border-border text-[0.72rem] text-text-bright hover:bg-surface-2"
          on:click={loadSensors}
        >
          {$_("monitor.loadSensors")}
        </button>
      </div>
    {/if}
  </div>

  <div class="bg-surface border border-border rounded p-4">
    <div class="flex items-center gap-1.5 mb-2">
      <Fan size={14} class="text-primary" />
      <span class="text-[0.78rem] text-text">{$_("monitor.fans")}</span>
    </div>
    {#if (snapshot?.fans ?? []).length > 0}
      <div class="max-h-[190px] overflow-y-auto overscroll-contain inner-scroll">
        <div class="mr-3">
          {#each snapshot?.fans ?? [] as sensor}
            <div class="py-1.5 border-t border-border">
              <div class="flex items-center justify-between text-[0.76rem]">
                <span class="text-text-bright truncate pr-3" title={`${sensor.chip} · ${sensor.label}`}>
                  {sensor.chip} · {sensor.label}
                </span>
                <span class="text-text shrink-0">{Math.round(sensor.value)} {$_("monitor.rpm")}</span>
              </div>
              <div class="h-1 w-full bg-surface-2 rounded-full mt-1 overflow-hidden">
                <div class="h-full rounded-full bg-primary" style={`width: ${fraction(sensor.value, 3000)}%`}></div>
              </div>
            </div>
          {/each}
        </div>
      </div>
    {:else}
      <p class="text-[0.76rem] text-text-muted">{$_("monitor.noSensors")}</p>
    {/if}
  </div>
</div>

<div class="bg-surface border border-border rounded p-4 mb-3.5">
  <div class="flex items-center gap-1.5 mb-2">
    <PlugZap size={14} class="text-primary" />
    <span class="text-[0.78rem] text-text">{$_("monitor.watts")}</span>
  </div>
  {#if (snapshot?.powers ?? []).length > 0}
    <div class="grid grid-cols-1 sm:grid-cols-2 gap-x-6">
      {#each snapshot?.powers ?? [] as sensor}
        <div class="py-1.5 border-t border-border">
          <div class="flex items-center justify-between text-[0.76rem]">
            <span class="text-text-bright truncate pr-3" title={`${sensor.chip} · ${sensor.label}`}>
              {sensor.chip} · {sensor.label}
            </span>
            <span class="text-text shrink-0 tabular-nums">{Math.round(sensor.value)} {$_("monitor.wattsUnit")}</span>
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <p class="text-[0.76rem] text-text-muted">{$_("monitor.noSensors")}</p>
  {/if}
</div>

<div class="bg-surface border border-border rounded p-4 mb-3.5">
  <div class="flex items-center gap-1.5 mb-2">
    <Zap size={14} class="text-primary" />
    <span class="text-[0.78rem] text-text">{$_("monitor.voltages")}</span>
  </div>
  {#if (snapshot?.voltages ?? []).length > 0}
    <div class="grid grid-cols-1 sm:grid-cols-2 gap-x-6">
      {#each snapshot?.voltages ?? [] as sensor}
        <div class="py-1.5 border-t border-border">
          <div class="flex items-center justify-between text-[0.76rem]">
            <span class="text-text-bright truncate pr-3" title={`${sensor.chip} · ${sensor.label}`}>
              {sensor.chip} · {sensor.label}
            </span>
            <span class="text-text shrink-0 tabular-nums">{sensor.value.toFixed(2)} {$_("monitor.volts")}</span>
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <p class="text-[0.76rem] text-text-muted">{$_("monitor.noSensors")}</p>
  {/if}
</div>

<div class="bg-surface border border-border rounded p-4">
  <div class="flex items-center gap-1.5 mb-2 text-text">
    <ListOrdered size={14} class="text-primary" />
    <span class="text-[0.78rem]">{$_("monitor.processes")}</span>
  </div>
  <div class="max-h-[340px] overflow-y-auto overscroll-contain inner-scroll">
    <div class="mr-3">
    <table class="w-full text-[0.76rem]">
    <thead class="text-text-muted sticky top-0 bg-surface">
      <tr class="text-left">
        <th class="font-normal pb-1">{$_("monitor.process")}</th>
        <th class="font-normal pb-1 w-16 text-right">{$_("monitor.cpu")}</th>
        <th class="font-normal pb-1 w-20 text-right">{$_("monitor.ram")}</th>
      </tr>
    </thead>
    <tbody>
      {#each (snapshot?.processes ?? []) as process (process.pid)}
        <tr class="border-t border-border">
          <td class="py-1 text-text-bright truncate max-w-[240px]" title={`PID ${process.pid}`}>{process.name}</td>
          <td class="py-1 text-right {process.cpu >= 50 ? 'text-danger' : 'text-text'}">{Math.round(process.cpu)} %</td>
          <td class="py-1 text-right text-text">{process.ramMb} MB</td>
        </tr>
      {:else}
        <tr><td class="py-2 text-text-muted" colspan="3">{$_("monitor.noData")}</td></tr>
      {/each}
    </tbody>
    </table>
    </div>
  </div>
</div>
