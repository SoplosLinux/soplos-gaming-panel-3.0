<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { _ } from "svelte-i18n";
  import { get } from "svelte/store";
  import {
    ChevronDown,
    Cpu,
    FileText,
    Gamepad2,
    Gauge,
    HardDrive,
    History,
    Monitor,
    Scale,
    SlidersHorizontal,
    Snowflake,
    Trophy,
    Zap,
  } from "lucide-svelte";
  import Switch from "../components/Switch.svelte";
  import {
    gamemodeIniRemove,
    gamemodeIniSet,
    gamemodeIniStatus,
    gamemodeState,
    governorApply,
    governorAuto,
    governorCatalogue,
    governorExclusion,
    governorManual,
    governorRestore,
    governorRule,
    governorService,
    governorServiceControl,
    governorStatus,
    installOverlayTools,
    installPackages,
    kernelsStatus,
    overlayToolsInstalled,
    ryzenadjInstalled,
    scxSet,
    scxStatus,
    tweakSet,
    tweaksRevertAll,
    tweaksStatus,
    type GameModeIni,
    type GameModeState,
    type GovernorAuto,
    type GovernorLever,
    type GovernorMissing,
    type GovernorStatus,
    type KernelInfo,
    type ScxState,
    type ServiceState,
    type TweakState,
  } from "../lib/api";

  let status: GovernorStatus | null = null;
  let levers: GovernorLever[] = [];
  let missing: GovernorMissing[] = [];
  let preview: Record<string, Record<string, string>> = {};
  let totals: { name: string; seconds: number }[] = [];
  let service: ServiceState = { unitInstalled: false, running: false };
  let gamemode: GameModeState = { installed: false, active: false };
  let ini: GameModeIni | null = null;
  let tweaks: TweakState[] = [];
  let overlay = false;
  let ryzenadj = false;
  let kernels: KernelInfo[] = [];
  let scx: ScxState = { kernel: false, service: false, current: "unknown", mode: 0, supported: [] };
  let showSchedulerHelp = false;

  /** What the README of each scheduler says it is for, in a few words: the
   * schedulers of scx 1.1.5 that scx-scheds ships. */
  const SCHEDULER_HELP = new Set([
    "scx_beerland",
    "scx_bpfland",
    "scx_cake",
    "scx_chaos",
    "scx_cosmos",
    "scx_eevdf",
    "scx_flash",
    "scx_flow",
    "scx_forge",
    "scx_lavd",
    "scx_layered",
    "scx_mavd",
    "scx_mitosis",
    "scx_mlfq",
    "scx_nitosis",
    "scx_p2dq",
    "scx_pandemonium",
    "scx_rburst",
    "scx_reevdf",
    "scx_rlocal",
    "scx_rusty",
    "scx_rvtime",
    "scx_tickless",
  ]);

  /** A mark next to the name: made with games in mind, still experimental, or
   * only for testing. The rest are general schedulers. */
  const SCHEDULER_TAGS: Record<string, "gaming" | "experimental" | "testing"> = {
    scx_bpfland: "gaming",
    scx_cake: "gaming",
    scx_lavd: "gaming",
    scx_eevdf: "experimental",
    scx_flow: "experimental",
    scx_mavd: "experimental",
    scx_mlfq: "experimental",
    scx_nitosis: "experimental",
    scx_rburst: "experimental",
    scx_reevdf: "experimental",
    scx_rlocal: "experimental",
    scx_rvtime: "experimental",
    scx_tickless: "experimental",
    scx_chaos: "testing",
  };
  const TAG_TINTS: Record<string, string> = {
    gaming: "bg-success/15 text-success",
    experimental: "bg-warning/15 text-warning",
    testing: "bg-danger/15 text-danger",
  };

  function aboutScheduler(name: string): string {
    return SCHEDULER_HELP.has(name) ? $_(`scx.about.${name}`) : $_("scx.aboutUnknown");
  }
  const scxModes = ["auto", "gaming", "powersave", "lowlatency", "server"];
  let error = "";
  // An error of something the user did stays until the next thing they do;
  // only an error of reaching the daemon goes away by itself when it answers.
  let errorOfAction = false;
  let busy = false;
  let details = false;
  let manualName = "";
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

  function emptyAuto(): GovernorAuto {
    return { enabled: false, profile: "max", exclusions: [], rules: [], manual: [] };
  }

  $: active = status?.active ?? false;
  $: profiles = status?.profiles ?? ["max", "balanced", "quiet"];
  $: auto = { ...emptyAuto(), ...(status?.auto ?? {}) };
  $: chosen = status?.profile ?? auto.profile;
  $: games = status?.games ?? [];
  $: session = status?.session ?? null;
  $: history = status?.history ?? [];
  $: activeKernel = kernels.find((one) => one.active) ?? null;
  $: pendingInstall = missing.filter((one) => one.fix.kind === "package");
  $: missingTweaks = tweaks.filter((one) => one.available && !one.on);

  async function refresh() {
    // The tick of the page asks only for what moves on its own: whether a
    // game is running and whether the service is up. The rest — levers,
    // optimizations, tools, kernels, scheduler — is read when the page opens
    // and after every action.
    await watchGame();
    await readEverything();
  }

  /** The two things that change without anybody pressing anything. */
  async function watchGame() {
    try {
      gamemode = await gamemodeState();
    } catch {
      gamemode = { installed: false, active: false };
    }
    try {
      status = await governorStatus();
      if (!errorOfAction) error = "";
    } catch {
      status = null;
    }
  }

  async function readEverything() {
    try {
      const catalogue = await governorCatalogue();
      levers = catalogue.levers;
      missing = catalogue.missing;
      preview = catalogue.preview ?? {};
      totals = catalogue.totals ?? [];
    } catch {
      // The page keeps what it last read.
    }
    try {
      service = await governorService();
    } catch {
      service = { unitInstalled: false, running: false };
    }
    try {
      gamemode = await gamemodeState();
    } catch {
      gamemode = { installed: false, active: false };
    }
    try {
      ini = await gamemodeIniStatus();
    } catch {
      ini = null;
    }
    try {
      tweaks = await tweaksStatus();
    } catch {
      tweaks = [];
    }
    try {
      overlay = await overlayToolsInstalled();
    } catch {
      overlay = false;
    }
    try {
      ryzenadj = await ryzenadjInstalled();
    } catch {
      ryzenadj = false;
    }
    try {
      kernels = (await kernelsStatus()).kernels;
    } catch {
      kernels = [];
    }
    try {
      scx = await scxStatus();
    } catch {
      scx = { kernel: false, service: false, current: "unknown", mode: 0, supported: [] };
    }
  }

  /** A dismissed password is not a failure of the machine. */
  function explain(reason: unknown): string {
    const text = String(reason);
    if (/dismissed|not given|126/i.test(text)) return get(_)("gamemode.cancelled");
    return text;
  }

  async function run(action: () => Promise<unknown>) {
    if (busy) return;
    busy = true;
    try {
      await action();
      error = "";
      errorOfAction = false;
    } catch (reason) {
      error = explain(reason);
      errorOfAction = true;
    }
    await refresh();
    busy = false;
  }

  async function serviceControl(start: boolean) {
    await run(() => governorServiceControl(start));
  }

  async function install(one: GovernorMissing) {
    if (one.fix.kind !== "package") return;
    const apt = one.fix.apt ? [one.fix.apt] : [];
    const flatpak = one.fix.flatpak ? [one.fix.flatpak] : [];
    await run(() => installPackages(apt, flatpak));
  }

  /** One button: the whole set, not only the profile. */
  async function toggleMode(on: boolean) {
    await run(async () => {
      if (on) {
        if (!service.running) await governorServiceControl(true);
        await governorApply(auto.profile);
        if (auto.enabled) await governorAuto(true, auto.profile);
      } else {
        await governorRestore();
      }
    });
  }

  /** Choosing how it goes starts the service if it has to. */
  async function chooseStrength(profile: string) {
    await run(async () => {
      if (!service.running) await governorServiceControl(true);
      await governorApply(profile);
      if (auto.enabled) await governorAuto(true, profile);
    });
  }

  async function toggleAuto(enabled: boolean) {
    await run(() => governorAuto(enabled, chosen));
  }

  async function exclude(name: string, excludeIt: boolean) {
    await run(() => governorExclusion(name, excludeIt));
  }

  function ruleOf(name: string): string {
    return auto.rules.find((rule) => rule.name === name)?.profile ?? "";
  }

  function schedulerOf(name: string): string {
    return auto.rules.find((rule) => rule.name === name)?.scheduler ?? "";
  }

  async function setRule(name: string, profile: string) {
    const scheduler = schedulerOf(name);
    await run(() => governorRule(name, profile, scheduler || null, scheduler ? 1 : null));
  }

  /** The scheduler of one game; with no name, it goes back to the kernel's own. */
  async function setGameScheduler(name: string, scheduler: string) {
    const profile = ruleOf(name) || auto.profile;
    await run(() => governorRule(name, profile, scheduler || null, scheduler ? 1 : null));
  }

  async function addManual() {
    const name = manualName.trim();
    if (!name) return;
    manualName = "";
    await run(() => governorManual(name, true));
  }

  async function removeManual(name: string) {
    await run(() => governorManual(name, false));
  }

  async function setTweak(key: string, on: boolean) {
    await run(() => tweakSet(key, on));
  }

  /** The scheduler is not a file of ours: the service of sched_ext does it. */
  async function chooseScheduler(event: Event) {
    const name = (event.currentTarget as HTMLSelectElement).value;
    await run(async () => {
      if (name) await scxSet(name, scx.mode);
      else await scxSet(null, 0);
      scx = await scxStatus();
    });
  }

  async function chooseScxMode(event: Event) {
    const mode = Number((event.currentTarget as HTMLSelectElement).value);
    await run(async () => {
      if (scx.current !== "unknown") await scxSet(scx.current, mode);
      scx = await scxStatus();
    });
  }

  async function revertTweaks() {
    await run(() => tweaksRevertAll());
  }

  async function installOverlay() {
    await run(() => installOverlayTools());
  }

  async function setIni(section: string, key: string, value: string, previous: string | null) {
    await run(async () => {
      ini =
        value.trim() === "" && previous !== null
          ? await gamemodeIniRemove(section, key)
          : await gamemodeIniSet(section, key, value.trim());
    });
  }

  /** "45 s", "12 min", "1 h 05 min": nobody reads 3900 seconds. */
  function duration(seconds: number): string {
    if (seconds < 60) return $_("gamemode.seconds", { values: { seconds: Math.round(seconds) } });
    const minutes = Math.round(seconds / 60);
    if (minutes < 60) return $_("gamemode.minutes", { values: { minutes } });
    return $_("gamemode.hoursMinutes", { values: { hours: Math.floor(minutes / 60), minutes: minutes % 60 } });
  }

  $: played = session ? duration(Math.max(1, Math.round(Date.now() / 1000) - session.seconds)) : "";

  const groups = ["cpu", "gpu", "memory", "kernel", "net", "disk"];
  $: sections = groups
    .map((group) => ({
      group,
      levers: levers.filter((lever) => lever.group === group),
      missing: missing.filter((one) => one.group === group),
    }))
    .filter((section) => section.levers.length > 0 || section.missing.length > 0);

  function labelOf(key: string): string {
    return $_(`lever.${key.replace(/\./g, "_")}`);
  }

  function amountOf(key: string, value: string): string {
    if (!key.endsWith("dirty_bytes")) return value;
    const bytes = Number(value);
    if (!Number.isFinite(bytes) || bytes <= 0) return value;
    return `${Math.round(bytes / 1048576)} MB`;
  }

  onMount(() => {
    refresh();
    // Only the watch of the game runs on its own; the rest is read when the
    // page opens and after each action.
    timer = setInterval(watchGame, 2000);
  });

  onDestroy(() => clearInterval(timer));
</script>

<div class="bg-surface border border-border rounded p-4 mb-3.5">
  <div class="flex items-center gap-4">
    <div class="relative shrink-0">
      <svg viewBox="0 0 44 44" class="w-[76px] h-[76px] -rotate-90">
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
        <Zap size={22} class={active ? "text-primary" : "text-text-muted"} />
      </span>
    </div>
    <div class="min-w-0 flex-1">
      <p class="text-[0.68rem] uppercase tracking-wide text-text-muted">{$_("gamemode.mode")}</p>
      <p class="text-[1.4rem] font-semibold text-text-bright leading-tight mt-0.5">
        {active ? $_("gamemode.ready") : $_("gamemode.idle")}
      </p>
      <p class="text-[0.78rem] text-text-muted mt-1 leading-snug">
        {active ? $_("gamemode.onText") : $_("gamemode.effect")}
      </p>
      {#if session}
        <p class="text-[0.78rem] text-primary mt-1.5">
          {$_("gamemode.playing", { values: { name: session.name } })} · {played}
        </p>
      {/if}
    </div>
  </div>

  <button
    class="w-full mt-4 py-2.5 rounded text-[0.9rem] font-medium transition-colors disabled:opacity-40 disabled:cursor-not-allowed {active
      ? 'border border-border text-text-bright hover:bg-surface-2'
      : 'bg-primary text-white hover:opacity-90'}"
    disabled={busy}
    on:click={() => (!service.running ? serviceControl(true) : toggleMode(!active))}
  >
    {!service.running ? $_("gamemode.prepare") : active ? $_("gamemode.turnOff") : $_("gamemode.turnOn")}
  </button>

  <div class="flex items-center justify-between gap-3 mt-3 pt-3 border-t border-border">
    <div class="min-w-0">
      <p class="text-[0.82rem] text-text-bright">{$_("gamemode.automatic")}</p>
      <p class="text-[0.74rem] text-text-muted mt-0.5">{$_("gamemode.automaticText")}</p>
    </div>
    <Switch checked={auto.enabled} disabled={busy} label={$_("gamemode.automatic")} on:change={(event) => toggleAuto(event.detail)} />
  </div>

  {#if error}
    <div class="border border-danger/50 bg-danger/10 rounded px-3 py-2 mt-3">
      <p class="text-[0.8rem] text-danger">{$_("gamemode.applyError")}</p>
      <!-- One lever per line when several could not go back. -->
      <p class="text-[0.74rem] text-text-muted mt-0.5 break-all whitespace-pre-line">{error}</p>
    </div>
  {/if}
</div>

<div class="bg-surface border border-border rounded p-4 mb-3.5">
  <div class="flex items-center gap-2">
    <Gauge size={16} class="text-primary shrink-0" />
    <span class="text-[0.9rem] text-text-bright">{$_("gamemode.strength")}</span>
  </div>
  <div class="grid grid-cols-3 gap-2 mt-3">
    {#each profiles as profile (profile)}
      <button
        class="flex flex-col items-center gap-2 rounded border p-2.5 text-center transition-colors disabled:opacity-40 {chosen === profile
          ? 'border-primary bg-primary/10'
          : 'border-border hover:bg-surface-2'}"
        disabled={busy}
        on:click={() => chooseStrength(profile)}
      >
        <span class="w-9 h-9 rounded-lg flex items-center justify-center {chosen === profile ? 'bg-primary/20 text-primary' : 'bg-surface-2 text-text-muted'}">
          <svelte:component this={profileIcons[profile]} size={18} />
        </span>
        <span class="text-[0.82rem] text-text-bright leading-tight">{$_(`profile.${profile}`)}</span>
        <span class="text-[0.7rem] text-text-muted leading-snug">{$_(`profile.${profile}Text`)}</span>
      </button>
    {/each}
  </div>
</div>

<div class="bg-surface border border-border rounded p-4 mb-3.5">
  <div class="flex items-center justify-between gap-3">
    <div class="flex items-center gap-2 min-w-0">
      <SlidersHorizontal size={16} class="text-primary shrink-0" />
      <div class="min-w-0">
        <p class="text-[0.9rem] text-text-bright">{$_("tweak.title")}</p>
        <p class="text-[0.74rem] text-text-muted mt-0.5">{$_("tweak.hint")}</p>
      </div>
    </div>
    <button
      class="shrink-0 px-2.5 py-1 rounded border border-border text-[0.74rem] text-text-bright hover:bg-surface-2 disabled:opacity-40"
      disabled={busy}
      on:click={revertTweaks}
    >
      {$_("tweak.revert")}
    </button>
  </div>

  <div class="grid grid-cols-5 gap-2 mt-3">
    {#each tweaks as tweak (tweak.key)}
      <button
        class="flex flex-col items-center gap-1.5 rounded border px-1 py-2.5 min-w-0 transition-colors disabled:cursor-not-allowed {tweak.on
          ? 'border-primary bg-primary/10'
          : 'border-border hover:bg-surface-2'} {tweak.available ? '' : 'opacity-45'}"
        disabled={busy || !tweak.available}
        title={tweak.available ? "" : $_("tweak.unavailable")}
        on:click={() => setTweak(tweak.key, !tweak.on)}
      >
        <span class="w-8 h-8 rounded-lg flex items-center justify-center {tweak.on ? 'bg-primary/20 text-primary' : 'bg-surface-2 text-text-muted'}">
          <svelte:component this={tweakIcons[tweak.key]} size={16} />
        </span>
        <span class="text-[0.68rem] leading-tight text-center {tweak.on ? 'text-text-bright' : 'text-text-muted'}">
          {$_(`tweak.${tweak.key}`)}
        </span>
      </button>
    {/each}
  </div>

  {#if missingTweaks.length > 0 || pendingInstall.length > 0}
    <p class="text-[0.76rem] text-text-muted mt-3">{$_("gamemode.pendingText")}</p>
  {/if}

  <div class="flex items-center justify-between gap-3 py-2 mt-2 border-t border-border">
    <div class="flex items-center gap-2 min-w-0">
      <span class="w-8 h-8 rounded-lg flex items-center justify-center shrink-0 {overlay ? 'bg-primary/20 text-primary' : 'bg-surface-2 text-text-muted'}">
        <Monitor size={15} />
      </span>
      <div class="min-w-0">
        <p class="text-[0.82rem] text-text-bright">{$_("tweak.overlay")}</p>
        <p class="text-[0.72rem] text-text-muted mt-0.5">{$_("tweak.overlayHint")}</p>
      </div>
    </div>
    {#if overlay}
      <span class="shrink-0 text-[0.74rem] text-primary">{$_("tweak.present")}</span>
    {:else}
      <button
        class="shrink-0 px-2.5 py-1 rounded border border-border text-[0.74rem] text-text-bright hover:bg-surface-2 disabled:opacity-40"
        disabled={busy}
        on:click={installOverlay}
      >
        {$_("gamemode.install")}
      </button>
    {/if}
  </div>

  <div class="flex items-center justify-between gap-3 py-2 border-t border-border">
    <div class="flex items-center gap-2 min-w-0">
      <span class="w-8 h-8 rounded-lg flex items-center justify-center shrink-0 {ryzenadj ? 'bg-primary/20 text-primary' : 'bg-surface-2 text-text-muted'}">
        <Cpu size={15} />
      </span>
      <div class="min-w-0">
        <p class="text-[0.82rem] text-text-bright">{$_("tweak.ryzenadj")}</p>
        <p class="text-[0.72rem] text-text-muted mt-0.5">{$_("tweak.ryzenadjHint")}</p>
      </div>
    </div>
    <span class="shrink-0 text-[0.74rem] {ryzenadj ? 'text-primary' : 'text-text-muted'}">
      {ryzenadj ? $_("tweak.present") : $_("gamemode.notInstalled")}
    </span>
  </div>
</div>

<div class="bg-surface border border-border rounded p-4 mb-3.5">
  <div class="flex items-center justify-between gap-3">
    <div class="flex items-center gap-2 min-w-0">
      <Gamepad2 size={16} class={gamemode.installed ? "text-primary" : "text-text-muted"} />
      <span class="text-[0.9rem] text-text-bright">{$_("gamemode.title")}</span>
    </div>
    {#if gamemode.installed}
      <span class="text-[0.78rem] {gamemode.active ? 'text-primary' : 'text-text-muted'}">
        {gamemode.active ? $_("gamemode.active") : $_("gamemode.inactive")}
      </span>
    {:else}
      <button
        class="shrink-0 px-2.5 py-1 rounded border border-border text-[0.76rem] text-text-bright hover:bg-surface-2 disabled:opacity-40"
        disabled={busy}
        on:click={() => run(() => installPackages(["gamemode"], []))}
      >
        {$_("gamemode.install")}
      </button>
    {/if}
  </div>

  <div class="mt-3 pt-3 border-t border-border">
    {#if games.length > 0}
      {#each games as game (game.pid)}
        <div class="flex flex-wrap items-center gap-2 py-2 border-b border-border">
          <span class="min-w-0 flex-1 truncate text-[0.82rem] text-text-bright">{game.name}</span>
          <select
            class="w-36 shrink-0 bg-surface-2 border border-border rounded px-1.5 py-1 text-[0.74rem] text-text-bright outline-none"
            value={ruleOf(game.name)}
            disabled={busy}
            on:change={(event) => setRule(game.name, event.currentTarget.value)}
          >
            <option value="">{$_("gamemode.ruleDefault")}</option>
            {#each profiles as profile (profile)}
              <option value={profile}>{$_(`profile.${profile}`)}</option>
            {/each}
          </select>
          <!-- The state comes from Rust as plain data: whether a scheduler can be
               chosen is the kernel and the service answering, both. -->
          {#if scx.kernel && scx.service && scx.supported.length > 0}
            <select
              class="w-32 shrink-0 bg-surface-2 border border-border rounded px-1.5 py-1 text-[0.74rem] text-text-bright outline-none"
              value={schedulerOf(game.name)}
              disabled={busy}
              title={$_("scx.scheduler")}
              on:change={(event) => setGameScheduler(game.name, event.currentTarget.value)}
            >
              <option value="">{$_("scx.none")}</option>
              {#each scx.supported as name (name)}
                <option value={name}>{name}</option>
              {/each}
            </select>
          {/if}
          <button
            class="shrink-0 px-2 py-0.5 rounded border border-border text-[0.72rem] text-text-muted hover:text-text-bright disabled:opacity-40"
            disabled={busy}
            on:click={() => exclude(game.name, true)}
          >
            {$_("gamemode.exclude")}
          </button>
        </div>
      {/each}
    {:else}
      <p class="text-[0.78rem] text-text-muted">{$_("gamemode.noGames")}</p>
    {/if}

    <div class="flex items-center gap-2 mt-3">
      <input
        class="flex-1 min-w-0 bg-surface-2 border border-border rounded px-2.5 py-1.5 text-[0.78rem] text-text-bright outline-none focus-visible:ring-2 focus-visible:ring-primary/60"
        placeholder={$_("gamemode.manualName")}
        bind:value={manualName}
        on:keydown={(event) => event.key === "Enter" && addManual()}
      />
      <button
        class="px-3 py-1.5 rounded border border-border text-[0.76rem] text-text-bright hover:bg-surface-2 disabled:opacity-40"
        disabled={busy || manualName.trim() === ""}
        on:click={addManual}
      >
        {$_("gamemode.manualAdd")}
      </button>
    </div>

    {#if auto.manual.length > 0}
      <div class="flex flex-wrap gap-1.5 mt-2">
        {#each auto.manual as name (name)}
          <button
            class="px-2 py-0.5 rounded border border-border text-[0.72rem] text-text-muted hover:text-text-bright"
            on:click={() => removeManual(name)}
            title={$_("gamemode.excludedHint")}
          >
            {name} ×
          </button>
        {/each}
      </div>
    {/if}

    {#if auto.rules.length > 0}
      <p class="text-[0.72rem] text-text-muted mt-3 mb-1">{$_("gamemode.rules")}</p>
      <div class="flex flex-wrap gap-1.5">
        {#each auto.rules as rule (rule.name)}
          <button
            class="px-2 py-0.5 rounded border border-border text-[0.72rem] text-text-muted hover:text-text-bright"
            on:click={() => setRule(rule.name, "")}
            title={$_("gamemode.excludedHint")}
          >
            {rule.name} · {$_(`profile.${rule.profile}`)} ×
          </button>
        {/each}
      </div>
    {/if}

    {#if auto.exclusions.length > 0}
      <p class="text-[0.72rem] text-text-muted mt-3 mb-1">{$_("gamemode.exclusions")}</p>
      <div class="flex flex-wrap gap-1.5">
        {#each auto.exclusions as name (name)}
          <button
            class="px-2 py-0.5 rounded border border-border text-[0.72rem] text-text-muted hover:text-text-bright"
            on:click={() => exclude(name, false)}
            title={$_("gamemode.excludedHint")}
          >
            {name} ×
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

<div class="bg-surface border border-border rounded p-4 mb-3.5">
  <div class="flex items-center gap-2">
    <Gauge size={16} class="text-primary shrink-0" />
    <div class="min-w-0">
      <p class="text-[0.9rem] text-text-bright">{$_("scx.title")}</p>
      <p class="text-[0.74rem] text-text-muted mt-0.5">{$_("scx.hint")}</p>
    </div>
  </div>

  {#if !scx.kernel}
    <p class="text-[0.78rem] text-text-muted mt-2.5">{$_("scx.noKernel")}</p>
  {:else if !scx.service}
    <p class="text-[0.78rem] text-text-muted mt-2.5">{$_("scx.noService")}</p>
  {:else}
    <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 mt-3">
      <label class="flex flex-col gap-1">
        <span class="text-[0.74rem] text-text-muted">{$_("scx.scheduler")}</span>
        <div class="relative">
          <select
            class="appearance-none w-full bg-surface-2 border border-border rounded pl-2.5 pr-8 h-8 text-[0.82rem] text-text-bright outline-none focus-visible:ring-2 focus-visible:ring-primary/60"
            value={scx.current === "unknown" ? "" : scx.current}
            on:change={chooseScheduler}
          >
            <option value="">{$_("scx.none")}</option>
            {#each scx.supported as name (name)}
              <option value={name}>{name}</option>
            {/each}
          </select>
          <ChevronDown size={14} class="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-text-muted" />
        </div>
      </label>
      <label class="flex flex-col gap-1">
        <span class="text-[0.74rem] text-text-muted">{$_("scx.mode")}</span>
        <div class="relative">
          <select
            class="appearance-none w-full bg-surface-2 border border-border rounded pl-2.5 pr-8 h-8 text-[0.82rem] text-text-bright outline-none focus-visible:ring-2 focus-visible:ring-primary/60"
            value={scx.mode}
            on:change={chooseScxMode}
          >
            {#each scxModes as mode, index}
              <option value={index}>{$_(`scx.modes.${mode}`)}</option>
            {/each}
          </select>
          <ChevronDown size={14} class="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-text-muted" />
        </div>
      </label>
    </div>

    <p class="text-[0.76rem] text-text-muted mt-2.5 leading-snug">
      {scx.current === "unknown" ? $_("scx.aboutNone") : aboutScheduler(scx.current)}
    </p>

    <!-- What each one is for, as its own authors describe it, so the choice
         is not a list of names. -->
    <button
      class="flex items-center gap-1.5 mt-3 text-[0.78rem] text-primary hover:underline"
      on:click={() => (showSchedulerHelp = !showSchedulerHelp)}
      aria-expanded={showSchedulerHelp}
    >
      <ChevronDown size={14} class="transition-transform {showSchedulerHelp ? '' : '-rotate-90'}" />
      {$_("scx.help")}
    </button>
    {#if showSchedulerHelp}
      <ul class="mt-2 flex flex-col divide-y divide-border border border-border rounded">
        {#each scx.supported as name (name)}
          <li class="px-3 py-2">
            <div class="flex items-center gap-2 flex-wrap">
              <span class="text-[0.8rem] text-text-bright font-medium">{name}</span>
              {#if SCHEDULER_TAGS[name]}
                <span class="text-[0.64rem] uppercase tracking-wide rounded px-1.5 py-0.5 {TAG_TINTS[SCHEDULER_TAGS[name]]}">
                  {$_(`scx.tags.${SCHEDULER_TAGS[name]}`)}
                </span>
              {/if}
              {#if name === scx.current}
                <span class="text-[0.64rem] uppercase tracking-wide rounded px-1.5 py-0.5 bg-primary/15 text-primary">{$_("scx.running")}</span>
              {/if}
            </div>
            <p class="text-[0.74rem] text-text-muted mt-0.5 leading-snug">{aboutScheduler(name)}</p>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>

<div class="bg-surface border border-border rounded p-4 mb-3.5">
  <div class="flex items-center gap-2">
    <Cpu size={16} class="text-primary shrink-0" />
    <div class="min-w-0">
      <p class="text-[0.9rem] text-text-bright">{$_("gamemode.kernel.title")}</p>
      <p class="text-[0.74rem] text-text-muted mt-0.5">{$_("gamemode.kernel.hint")}</p>
    </div>
  </div>

  {#if kernels.length === 0}
    <p class="text-[0.78rem] text-text-muted mt-2.5">{$_("gamemode.kernel.none")}</p>
  {:else}
    <div class="mt-3 border border-border rounded overflow-hidden">
      {#each kernels as kernel (kernel.version)}
        <div class="flex items-center gap-3 px-3 py-2 border-t border-border first:border-t-0">
          <div class="min-w-0 flex-1">
            <p class="text-[0.8rem] text-text-bright truncate">{kernel.version}</p>
            <div class="flex flex-wrap gap-1.5 mt-1">
              {#if kernel.schedExt === false}
                <span class="px-1.5 py-0.5 rounded border border-border text-[0.68rem] text-text-muted">{$_("gamemode.kernel.noSchedExt")}</span>
              {/if}
              {#if kernel.btf === false}
                <span class="px-1.5 py-0.5 rounded border border-border text-[0.68rem] text-text-muted">{$_("gamemode.kernel.noBtf")}</span>
              {/if}
              {#if kernel.schedExt === null && kernel.btf === null}
                <span class="px-1.5 py-0.5 rounded border border-border text-[0.68rem] text-text-muted">{$_("gamemode.kernel.noConfig")}</span>
              {/if}
            </div>
          </div>
          {#if kernel.active}
            <span class="shrink-0 px-2 py-0.5 rounded-full border border-primary text-[0.72rem] text-primary">
              {$_("gamemode.kernel.active")}
            </span>
          {/if}
        </div>
      {/each}
    </div>
    {#if activeKernel && (activeKernel.schedExt === false || activeKernel.btf === false)}
      <p class="text-[0.74rem] text-text-muted mt-2">{$_("gamemode.kernel.plainHint")}</p>
    {/if}
  {/if}
</div>

{#if totals.length > 0}
  <div class="bg-surface border border-border rounded p-4 mb-3.5">
    <div class="flex items-center gap-2">
      <Trophy size={16} class="text-primary shrink-0" />
      <span class="text-[0.9rem] text-text-bright">{$_("gamemode.mostPlayed")}</span>
    </div>
    <div class="mt-2">
      {#each totals as total, index (total.name)}
        <div class="flex items-center gap-3 py-1.5 border-b border-border last:border-b-0">
          <span class="w-4 shrink-0 text-[0.74rem] text-text-muted tabular-nums">{index + 1}</span>
          <span class="flex-1 min-w-0 truncate text-[0.82rem] text-text-bright">{total.name}</span>
          <span class="w-24 text-right text-[0.78rem] tabular-nums {index === 0 ? 'text-primary' : 'text-text'}">{duration(total.seconds)}</span>
        </div>
      {/each}
    </div>
  </div>
{/if}

<div class="bg-surface border border-border rounded p-4">
  <div class="flex items-center gap-2">
    <History size={16} class="text-primary shrink-0" />
    <span class="text-[0.9rem] text-text-bright">{$_("gamemode.history")}</span>
  </div>
  {#if history.length > 0}
    <div class="mt-2 border border-border rounded overflow-hidden">
      <div class="flex items-center gap-3 px-3 py-1.5 bg-surface-2 text-[0.68rem] uppercase tracking-wide text-text-muted">
        <span class="flex-1">{$_("gamemode.colGame")}</span>
        <span class="w-32 text-right">{$_("gamemode.colProfile")}</span>
        <span class="w-24 text-right">{$_("gamemode.colTime")}</span>
      </div>
      {#each history as past (past.at + past.name)}
        <div class="flex items-center gap-3 px-3 py-2 border-t border-border text-[0.8rem]">
          <span class="flex-1 min-w-0 truncate text-text-bright">{past.name}</span>
          <span class="w-32 text-right text-text-muted truncate">{$_(`profile.${past.profile}`)}</span>
          <span class="w-24 text-right text-text tabular-nums">{duration(past.seconds)}</span>
        </div>
      {/each}
    </div>
  {:else}
    <p class="text-[0.78rem] text-text-muted mt-2">{$_("gamemode.noHistory")}</p>
  {/if}

  <button
    class="flex items-center justify-between gap-3 w-full text-left mt-3 pt-3 border-t border-border"
    on:click={() => (details = !details)}
  >
    <span class="flex items-center gap-2">
      <SlidersHorizontal size={15} class="text-text-muted shrink-0" />
      <span class="text-[0.82rem] text-text">{$_("gamemode.details")}</span>
    </span>
    <ChevronDown size={14} class="text-text-muted shrink-0 {details ? 'rotate-180' : ''}" />
  </button>

  {#if details}
    <div class="mt-3">
      {#each sections as section (section.group)}
        <div class="mb-3 border border-border rounded overflow-hidden">
          <div class="flex items-center gap-3 px-2 py-1.5 bg-surface-2 text-[0.68rem] uppercase tracking-wide text-text-muted">
            <span class="flex-1">{$_(`group.${section.group}`)}</span>
            <span class="w-28 text-right normal-case">{$_("gamemode.now")}</span>
            <span class="w-28 text-right normal-case">{$_("gamemode.whilePlaying")}</span>
          </div>
          {#each section.levers as lever (lever.key)}
            <div class="flex items-center gap-3 px-2 py-1.5 border-t border-border text-[0.78rem]">
              <span class="flex-1 min-w-0 truncate text-text-bright" title={`${lever.key} · ${lever.path}`}>
                {labelOf(lever.key)}
              </span>
              <span class="w-28 text-right text-text tabular-nums">{amountOf(lever.key, lever.value)}</span>
              <span class="w-28 text-right tabular-nums {lever.value === lever.gameValue ? 'text-primary' : 'text-text-muted'}">
                {amountOf(lever.key, lever.gameValue)}
              </span>
            </div>
          {/each}
          {#each section.missing as one (one.key)}
            <div class="flex items-center gap-3 px-2 py-1.5 border-t border-border text-[0.78rem]">
              <span class="flex-1 min-w-0 truncate text-text-muted" title={one.key}>{labelOf(one.key)}</span>
              <span class="w-40 text-right text-[0.7rem] text-text-muted">{$_(`reason.${one.reason}`)}</span>
              <span class="w-28 text-right">
                {#if one.fix.kind === "package"}
                  <button
                    class="px-2 py-0.5 rounded border border-border text-[0.68rem] text-text-bright hover:bg-surface-2 disabled:opacity-40"
                    disabled={busy}
                    on:click={() => install(one)}
                  >
                    {$_("gamemode.install")}
                  </button>
                {:else if one.fix.kind === "kernel"}
                  <span class="text-[0.68rem] text-text-muted">{$_("gamemode.needsKernel")}</span>
                {:else}
                  <span class="text-[0.68rem] text-text-muted">{$_("gamemode.needsHardware")}</span>
                {/if}
              </span>
            </div>
          {/each}
        </div>
      {/each}

      {#if ini}
        <div class="flex items-center gap-2 mt-4">
          <FileText size={15} class="text-primary shrink-0" />
          <span class="text-[0.84rem] text-text-bright">{$_("gamemode.iniTitle")}</span>
          <span class="ml-auto text-[0.7rem] text-text-muted truncate max-w-[45%]" title={ini.path}>{ini.path}</span>
        </div>
        {#each ["general", "gpu"] as section (section)}
          <div class="mt-2 border border-border rounded overflow-hidden">
            <p class="px-3 py-1.5 bg-surface-2 text-[0.68rem] uppercase tracking-wide text-text-muted">
              {$_(`gamemode.ini_${section}`)}
            </p>
            {#each ini.settings.filter((one) => one.section === section) as setting (setting.key)}
              <div class="flex items-center gap-3 px-3 py-1.5 border-t border-border text-[0.78rem]">
                <span class="flex-1 min-w-0 truncate text-text-bright" title={`${setting.section}.${setting.key}`}>{setting.key}</span>
                <span class="w-16 text-right text-[0.7rem] text-text-muted">
                  {setting.value === null ? $_("gamemode.iniDefault") : ""}
                </span>
                {#if setting.choices.length > 0}
                  <select
                    class="w-40 bg-surface-2 border border-border rounded px-1.5 py-1 text-[0.74rem] text-text-bright outline-none"
                    value={setting.value ?? setting.default}
                    disabled={busy}
                    on:change={(event) => setIni(section, setting.key, event.currentTarget.value, setting.value)}
                  >
                    {#each setting.choices as choice (choice)}
                      <option value={choice}>{choice}</option>
                    {/each}
                  </select>
                {:else}
                  <input
                    class="w-40 bg-surface-2 border border-border rounded px-1.5 py-1 text-[0.74rem] text-text-bright outline-none"
                    value={setting.value ?? ""}
                    placeholder={setting.default}
                    disabled={busy}
                    on:change={(event) => setIni(section, setting.key, event.currentTarget.value, setting.value)}
                  />
                {/if}
              </div>
            {/each}
          </div>
        {/each}
        {#if ini.system}
          <p class="text-[0.72rem] text-text-muted mt-2">{$_("gamemode.iniSystem")}</p>
        {/if}
      {/if}
    </div>
  {/if}
</div>
