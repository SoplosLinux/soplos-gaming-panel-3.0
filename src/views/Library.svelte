<script lang="ts">
  import { onMount } from "svelte";
  import { _ } from "svelte-i18n";
  import { open } from "@tauri-apps/plugin-dialog";
  import { Gamepad2, ImagePlus, LayoutGrid, List, Play, Search, Trash2 } from "lucide-svelte";
  import {
    governorRule,
    governorStatus,
    libraryClearCover,
    libraryLaunch,
    libraryList,
    librarySetCover,
    scxStatus,
    type GameEntry,
    type GovernorStatus,
    type ScxState,
  } from "../lib/api";

  const profiles = ["max", "balanced", "quiet"];
  const pictureExtensions = ["png", "jpg", "jpeg", "webp"];

  let games: GameEntry[] = [];
  let status: GovernorStatus | null = null;
  let scx: ScxState | null = null;
  let query = "";
  let covers = true;
  let busy = "";
  let error = "";
  let loaded = false;

  /** A game as the list shows it: with the time played and its rule. */
  type ShownGame = GameEntry & { seconds: number; profile: string; scheduler: string };

  /** The name as it is compared: a program of /proc has no spaces nor case. */
  function plain(name: string): string {
    return name.toLowerCase().replace(/[^a-z0-9]/g, "");
  }

  /** The games shown, each with the time played and the rule it has. Both come
   * from the daemon, so they are folded in here: Svelte does not watch what a
   * function reads. */
  let shown: ShownGame[] = [];
  $: shown = games
    .filter((game) => game.name.toLowerCase().includes(query.trim().toLowerCase()))
    .map((game) => {
      const rule = (status?.auto.rules ?? []).find((one) => one.name === game.process);
      return {
        ...game,
        seconds: (status?.totals ?? []).find((one) => plain(one.name) === plain(game.process ?? game.name))?.seconds ?? 0,
        profile: rule?.profile ?? "",
        scheduler: rule?.scheduler ?? "",
      };
    });

  /** Where a game comes from: the names of the stores are not translated. */
  function sourceName(source: GameEntry["source"]): string {
    if (source === "steam") return "Steam";
    if (source === "lutris") return "Lutris";
    return $_("library.menu");
  }

  function time(seconds: number): string {
    if (seconds < 60) return "—";
    if (seconds < 3600) return $_("library.timeMinutes", { values: { minutes: Math.round(seconds / 60) } });
    return $_("library.timeHours", { values: { hours: (seconds / 3600).toFixed(1) } });
  }

  async function load() {
    try {
      games = await libraryList();
      error = "";
    } catch (reason) {
      error = String(reason);
    }
    try {
      status = await governorStatus();
    } catch {
      status = null;
    }
    try {
      scx = await scxStatus();
    } catch {
      scx = null;
    }
    loaded = true;
  }

  async function play(game: GameEntry) {
    busy = game.id;
    try {
      await libraryLaunch(game.id);
      error = "";
    } catch (reason) {
      error = String(reason);
    }
    busy = "";
  }

  /** The picture the user chooses is copied next to the others of the panel:
   * moving the original afterwards does not take the cover away. */
  async function chooseCover(game: GameEntry) {
    let chosen: string | null = null;
    try {
      const picked = await open({ multiple: false, filters: [{ name: "Imagen", extensions: pictureExtensions }] });
      chosen = Array.isArray(picked) ? (picked[0] ?? null) : picked;
    } catch {
      return;
    }
    if (!chosen) return;
    busy = game.id;
    try {
      await librarySetCover(game.id, chosen);
      await load();
    } catch (reason) {
      error = String(reason);
    }
    busy = "";
  }

  async function removeCover(game: GameEntry) {
    busy = game.id;
    try {
      await libraryClearCover(game.id);
      await load();
    } catch (reason) {
      error = String(reason);
    }
    busy = "";
  }

  async function setRule(game: ShownGame, profile: string) {
    if (!game.process) return;
    busy = game.id;
    try {
      const scheduler = game.scheduler || null;
      await governorRule(game.process, profile, scheduler, scheduler ? 1 : null);
      await load();
    } catch (reason) {
      error = String(reason);
    }
    busy = "";
  }

  async function setScheduler(game: ShownGame, scheduler: string) {
    if (!game.process) return;
    busy = game.id;
    try {
      const profile = game.profile || status?.auto.profile || "max";
      await governorRule(game.process, profile, scheduler || null, scheduler ? 1 : null);
      await load();
    } catch (reason) {
      error = String(reason);
    }
    busy = "";
  }

  onMount(load);
</script>

<div class="flex flex-wrap items-center gap-2 mb-3.5">
  <label class="flex items-center gap-2 flex-1 min-w-[220px] bg-surface border border-border rounded px-2.5 py-1.5">
    <Search size={14} class="text-text-muted shrink-0" />
    <input
      type="text"
      class="flex-1 min-w-0 bg-transparent outline-none text-[0.82rem] text-text-bright placeholder:text-text-muted"
      placeholder={$_("library.search")}
      bind:value={query}
    />
  </label>
  <button
    class="shrink-0 p-1.5 rounded border border-border text-text-muted hover:text-text-bright"
    title={covers ? $_("library.list") : $_("library.grid")}
    on:click={() => (covers = !covers)}
  >
    {#if covers}<List size={16} />{:else}<LayoutGrid size={16} />{/if}
  </button>
</div>

{#if error}
  <div class="border border-danger/50 bg-danger/10 rounded px-3 py-2 mb-3">
    <p class="text-[0.8rem] text-danger">{$_("library.error")}</p>
    <p class="text-[0.74rem] text-text-muted mt-0.5 break-all">{error}</p>
  </div>
{/if}

{#if loaded && shown.length === 0}
  <div class="flex flex-col items-center justify-center text-center py-14">
    <Gamepad2 size={40} class="text-text-muted mb-3" />
    <p class="text-[0.95rem] text-text-bright">
      {query.trim() ? $_("library.noMatch") : $_("library.empty")}
    </p>
    <p class="text-[0.78rem] text-text-muted mt-1 max-w-[420px]">
      {query.trim() ? $_("library.noMatchHint") : $_("library.emptyHint")}
    </p>
  </div>
{:else if covers}
  <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-3">
    {#each shown as game (game.id)}
      <div class="rounded-lg overflow-hidden border border-border bg-surface">
        <div class="aspect-[3/4] bg-dark/40 flex items-center justify-center overflow-hidden">
          {#if game.cover}
            <img src={game.cover} alt={game.name} class="w-full h-full object-cover" />
          {:else}
            <Gamepad2 size={34} class="text-text-muted" />
          {/if}
        </div>
        <div class="p-2">
          <p class="text-[0.8rem] text-text-bright truncate" title={game.name}>{game.name}</p>
          <span class="block text-[0.66rem] text-text-muted truncate">
            {sourceName(game.source)}
            {#if game.seconds >= 60}· {time(game.seconds)}{/if}
          </span>
          <div class="flex items-center gap-1 mt-1.5">
            <button
              class="flex-1 flex items-center justify-center gap-1 py-1 rounded bg-primary text-white text-[0.74rem] hover:opacity-90 disabled:opacity-40"
              disabled={busy === game.id}
              on:click={() => play(game)}
            >
              <Play size={12} />{$_("library.play")}
            </button>
            <button
              class="p-1 rounded border border-border text-text-muted hover:text-text-bright disabled:opacity-40"
              title={$_("library.cover")}
              disabled={busy === game.id}
              on:click={() => chooseCover(game)}
            >
              <ImagePlus size={13} />
            </button>
            {#if game.cover && game.source === "menu"}
              <button
                class="p-1 rounded border border-border text-text-muted hover:text-danger disabled:opacity-40"
                title={$_("library.coverClear")}
                disabled={busy === game.id}
                on:click={() => removeCover(game)}
              >
                <Trash2 size={13} />
              </button>
            {/if}
          </div>
        </div>
      </div>
    {/each}
  </div>
{:else}
  <div class="border border-border rounded bg-surface">
    {#each shown as game (game.id)}
      <div class="flex flex-wrap items-center gap-3 px-3 py-2 border-b border-border last:border-0">
        <span class="w-9 h-9 shrink-0 rounded bg-dark/40 flex items-center justify-center overflow-hidden">
          {#if game.icon || game.cover}
            <img src={game.cover ?? game.icon} alt={game.name} class="w-full h-full object-cover" />
          {:else}
            <Gamepad2 size={17} class="text-text-muted" />
          {/if}
        </span>
        <div class="min-w-0 flex-1">
          <p class="text-[0.84rem] text-text-bright truncate" title={game.name}>{game.name}</p>
          <p class="text-[0.68rem] text-text-muted truncate">
            {sourceName(game.source)}
            {#if game.seconds >= 60}· {time(game.seconds)}{/if}
          </p>
        </div>
        {#if game.process && scx && scx.kernel && scx.supported.length > 0}
          <select
            class="w-28 shrink-0 bg-surface-2 border border-border rounded px-1.5 py-1 text-[0.72rem] text-text-bright"
            title={$_("library.scheduler")}
            value={game.scheduler}
            disabled={busy === game.id}
            on:change={(event) => setScheduler(game, event.currentTarget.value)}
          >
            <option value="">{$_("scx.none")}</option>
            {#each scx.supported as name (name)}
              <option value={name}>{name}</option>
            {/each}
          </select>
        {/if}
        {#if game.process}
          <select
            class="w-32 shrink-0 bg-surface-2 border border-border rounded px-1.5 py-1 text-[0.72rem] text-text-bright"
            title={$_("library.rule")}
            value={game.profile}
            disabled={busy === game.id}
            on:change={(event) => setRule(game, event.currentTarget.value)}
          >
            <option value="">{$_("library.ruleDefault")}</option>
            {#each profiles as profile (profile)}
              <option value={profile}>{$_(`profile.${profile}`)}</option>
            {/each}
          </select>
        {/if}
        <button
          class="shrink-0 p-1.5 rounded border border-border text-text-muted hover:text-text-bright disabled:opacity-40"
          title={$_("library.cover")}
          disabled={busy === game.id}
          on:click={() => chooseCover(game)}
        >
          <ImagePlus size={14} />
        </button>
        {#if game.cover && game.source === "menu"}
          <button
            class="shrink-0 p-1.5 rounded border border-border text-text-muted hover:text-danger disabled:opacity-40"
            title={$_("library.coverClear")}
            disabled={busy === game.id}
            on:click={() => removeCover(game)}
          >
            <Trash2 size={14} />
          </button>
        {/if}
        <button
          class="shrink-0 flex items-center gap-1 px-2.5 py-1 rounded bg-primary text-white text-[0.76rem] hover:opacity-90 disabled:opacity-40"
          disabled={busy === game.id}
          on:click={() => play(game)}
        >
          <Play size={12} />{$_("library.play")}
        </button>
      </div>
    {/each}
  </div>
{/if}
