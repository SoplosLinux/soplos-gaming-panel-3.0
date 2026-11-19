<script lang="ts">
  import { onMount } from "svelte";
  import { _ } from "svelte-i18n";
  import { ChevronDown, FileCog, Plus, Trash2, Wand2, Activity } from "lucide-svelte";
  import {
    mangohudFpsLog,
    mangohudFpsLogOff,
    mangohudPreset,
    mangohudRemove,
    mangohudSet,
    mangohudStatus,
    mangohudText,
    type MangoHudSetting,
    type MangoHudState,
  } from "../lib/api";

  let state: MangoHudState | null = null;
  let file = "";
  let showFile = false;
  let error = "";
  let busy = false;
  let newKey = "";
  let newValue = "";

  const presets = ["performance", "readable", "minimal"];
  const groups: MangoHudSetting["group"][] = ["metrics", "look", "limits", "keys"];

  /** Which sections are open. The one of the metrics starts open because it is
   * what a player changes most. */
  let open: Record<string, boolean> = { metrics: true, look: false, limits: false, keys: false, file: false };

  function toggleSection(key: string) {
    open = { ...open, [key]: !open[key] };
  }

  /** The value the file has for a key, or null when it has none. The whole
   * table is built in a reactive sentence: a comparison inside a function is
   * not something Svelte can watch. */
  $: rows = groups.map((group) => {
    const settings = (state?.settings ?? [])
      .filter((setting) => setting.group === group)
      .map((setting) => ({
        setting,
        current: (state?.entries ?? []).find(([key]) => key === setting.key)?.[1] ?? null,
      }));
    return { group, settings, marked: settings.filter((one) => one.current != null).length };
  });

  /** `frame_timing` reads as "Frame timing", which is what the file says. */
  function label(key: string): string {
    const words = key.replace(/_/g, " ");
    return words.charAt(0).toUpperCase() + words.slice(1);
  }

  async function run(action: () => Promise<MangoHudState>) {
    if (busy) return;
    busy = true;
    try {
      state = await action();
      error = "";
      if (showFile) file = (await mangohudText()).text;
    } catch (reason) {
      error = String(reason);
    }
    busy = false;
  }

  /** Ticking a box writes the key alone for an option that is only on or off,
   * or the key with its default value; taking the box away takes the key away. */
  function toggle(setting: MangoHudSetting, on: boolean) {
    if (on) {
      void run(() => mangohudSet([[setting.key, setting.default]]));
    } else {
      void run(() => mangohudRemove(setting.key));
    }
  }

  function write(setting: MangoHudSetting, value: string) {
    void run(() => mangohudSet([[setting.key, value]]));
  }

  function addKey() {
    const key = newKey.trim();
    if (!key) return;
    const value = newValue.trim();
    void run(() => mangohudSet([[key, value]])).then(() => {
      newKey = "";
      newValue = "";
    });
  }

  async function toggleFile() {
    showFile = !showFile;
    if (showFile) {
      try {
        file = (await mangohudText()).text;
      } catch (reason) {
        error = String(reason);
      }
    }
  }

  onMount(async () => {
    try {
      state = await mangohudStatus();
    } catch (reason) {
      error = String(reason);
    }
  });
</script>

<div class="bg-surface border border-border rounded p-4 mb-3.5">
  <div class="flex items-start justify-between gap-3">
    <div class="flex items-start gap-2.5 min-w-0">
      <FileCog size={18} class="text-primary shrink-0 mt-0.5" />
      <div class="min-w-0">
        <p class="text-[0.95rem] text-text-bright leading-tight">{$_("mangohud.title")}</p>
        <p class="text-[0.74rem] text-text-muted mt-0.5 break-all">{state?.path ?? "—"}</p>
      </div>
    </div>
    <button class="flex items-center gap-1 text-[0.74rem] text-text-muted hover:text-text-bright shrink-0" on:click={toggleFile}>
      <ChevronDown size={14} class={showFile ? "rotate-180" : ""} />
      {showFile ? $_("mangohud.hideFile") : $_("mangohud.showFile")}
    </button>
  </div>

  {#if error}
    <div class="border border-danger/50 bg-danger/10 rounded px-3 py-2 mt-3">
      <p class="text-[0.8rem] text-danger">{$_("mangohud.error")}</p>
      <p class="text-[0.74rem] text-text-muted mt-0.5 break-all">{error}</p>
    </div>
  {/if}

  {#if state && !state.exists}
    <p class="text-[0.74rem] text-text-muted mt-3">{$_("mangohud.missing")}</p>
  {/if}

  <!-- The FPS of the overlay come from a log of MangoHud, and MangoHud only
       writes one when its file names it. This is that key. -->
  <div class="mt-4 rounded border border-border p-3">
    <div class="flex items-start gap-2.5">
      <Activity size={16} class="text-primary shrink-0 mt-0.5" />
      <div class="min-w-0 flex-1">
        <p class="text-[0.86rem] text-text-bright">{$_("mangohud.fpsLog")}</p>
        <p class="text-[0.72rem] text-text-muted mt-0.5 leading-snug">
          {state?.fpsLog ? $_("mangohud.fpsLogOn", { values: { path: state.fpsLog } }) : $_("mangohud.fpsLogHint")}
        </p>
      </div>
      {#if state?.fpsLog}
        <button
          class="shrink-0 px-2.5 py-1 rounded border border-border text-[0.74rem] text-text-bright hover:bg-surface-2 disabled:opacity-40"
          disabled={busy}
          on:click={() => run(() => mangohudFpsLogOff())}
        >
          {$_("mangohud.fpsLogOff")}
        </button>
      {:else}
        <button
          class="shrink-0 px-2.5 py-1 rounded bg-primary text-white text-[0.74rem] hover:opacity-90 disabled:opacity-40"
          disabled={busy}
          on:click={() => run(() => mangohudFpsLog())}
        >
          {$_("mangohud.fpsLogEnable")}
        </button>
      {/if}
    </div>
  </div>

  <div class="text-[0.66rem] text-text-muted mt-4 mb-1.5 uppercase tracking-wide">{$_("mangohud.presets")}</div>
  <div class="grid grid-cols-3 gap-2">
    {#each presets as preset (preset)}
      <button
        class="text-left rounded border border-border p-3 hover:bg-surface-2 disabled:opacity-40 transition-colors {state?.preset === preset
          ? 'border-primary bg-primary/10'
          : ''}"
        disabled={busy}
        on:click={() => run(() => mangohudPreset(preset))}
      >
        <span class="flex items-center gap-1.5 text-[0.84rem] text-text-bright">
          <Wand2 size={13} class="text-primary" />{$_(`mangohud.preset_${preset}`)}
        </span>
        <span class="block text-[0.72rem] text-text-muted mt-1 leading-snug">{$_(`mangohud.preset_${preset}Text`)}</span>
      </button>
    {/each}
  </div>

  {#each rows as { group, settings, marked } (group)}
    <section class="mt-2.5 border border-border rounded bg-surface overflow-hidden">
      <button
        type="button"
        class="w-full flex items-center gap-2 px-3 py-2.5 text-left hover:bg-surface-2 transition-colors"
        on:click={() => toggleSection(group)}
      >
        <ChevronDown
          size={15}
          class="text-text-muted shrink-0 transition-transform {open[group] ? '' : '-rotate-90'}"
        />
        <span class="flex-1 min-w-0 text-[0.86rem] text-text-bright">{$_(`mangohud.groups.${group}`)}</span>
        <span class="shrink-0 text-[0.7rem] text-text-muted tabular-nums">{marked}/{settings.length}</span>
      </button>
      {#if open[group]}
        <div class="px-3 pb-3 border-t border-border">
          <div class="grid grid-cols-2 lg:grid-cols-3 gap-x-4 gap-y-1.5 mt-2.5">
            {#each settings as { setting, current } (setting.key)}
              <div class="flex items-center gap-2 min-w-0">
                {#if setting.kind === "flag"}
                  <label class="flex items-center gap-2 text-[0.8rem] min-w-0 cursor-pointer">
                    <input
                      type="checkbox"
                      class="accent-[#ff7d2e] shrink-0"
                      checked={current != null}
                      disabled={busy}
                      on:change={(event) => toggle(setting, event.currentTarget.checked)}
                    />
                    <span class="text-text truncate" title={setting.key}>{label(setting.key)}</span>
                  </label>
                {:else}
                  <span class="text-[0.8rem] text-text truncate" title={setting.key}>{label(setting.key)}</span>
                  {#if setting.choices.length > 0}
                    <select
                      class="ml-auto shrink-0 bg-surface-2 border border-border rounded px-1.5 py-0.5 text-[0.76rem] text-text-bright"
                      disabled={busy}
                      value={current ?? ""}
                      title={setting.key}
                      on:change={(event) => write(setting, event.currentTarget.value)}
                    >
                      <option value="">—</option>
                      {#each setting.choices as choice (choice)}
                        <option value={choice}>{choice}</option>
                      {/each}
                    </select>
                  {:else if setting.kind === "number"}
                    <input
                      type="number"
                      class="ml-auto w-[68px] shrink-0 bg-surface-2 border border-border rounded px-1.5 py-0.5 text-[0.76rem] text-text-bright tabular-nums"
                      min={setting.min ?? undefined}
                      max={setting.max ?? undefined}
                      step={setting.max != null && setting.max <= 1 ? 0.05 : 1}
                      value={current ?? ""}
                      title={setting.key}
                      disabled={busy}
                      on:change={(event) => {
                        const value = event.currentTarget.value.trim();
                        if (value) write(setting, value);
                      }}
                    />
                  {:else}
                    <input
                      type="text"
                      class="ml-auto w-[110px] shrink-0 bg-surface-2 border border-border rounded px-1.5 py-0.5 text-[0.76rem] text-text-bright"
                      value={current ?? ""}
                      placeholder="—"
                      title={setting.key}
                      disabled={busy}
                      on:change={(event) => {
                        const value = event.currentTarget.value.trim();
                        if (value) write(setting, value);
                      }}
                    />
                  {/if}
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </section>
  {/each}

  <!-- The keys of the file, the ones the window offers and the ones it does
       not, with a way of adding any of them by hand. -->
  <section class="mt-2.5 mb-3.5 border border-border rounded bg-surface overflow-hidden">
    <button
      type="button"
      class="w-full flex items-center gap-2 px-3 py-2.5 text-left hover:bg-surface-2 transition-colors"
      on:click={() => toggleSection("file")}
    >
      <ChevronDown size={15} class="text-text-muted shrink-0 transition-transform {open.file ? '' : '-rotate-90'}" />
      <span class="flex-1 min-w-0 text-[0.86rem] text-text-bright">{$_("mangohud.entries")}</span>
      <span class="shrink-0 text-[0.7rem] text-text-muted tabular-nums">{state?.entries.length ?? 0}</span>
    </button>
    {#if open.file}
      <div class="px-3 pb-3 border-t border-border">
        <div class="flex flex-wrap items-center gap-2 mt-2.5 mb-2.5">
          <input
            type="text"
            class="w-[150px] bg-surface-2 border border-border rounded px-2 py-1 text-[0.78rem] text-text-bright"
            placeholder={$_("mangohud.keyName")}
            bind:value={newKey}
          />
          <input
            type="text"
            class="w-[150px] bg-surface-2 border border-border rounded px-2 py-1 text-[0.78rem] text-text-bright"
            placeholder={$_("mangohud.keyValue")}
            bind:value={newValue}
          />
          <button
            class="flex items-center gap-1 px-2.5 py-1 rounded border border-border text-[0.78rem] text-text-bright hover:bg-surface-2 disabled:opacity-40"
            disabled={busy || !newKey.trim()}
            on:click={addKey}
          >
            <Plus size={13} />{$_("mangohud.addKey")}
          </button>
        </div>

        {#if state && state.entries.length === 0}
          <p class="text-[0.76rem] text-text-muted">{$_("mangohud.noEntries")}</p>
        {:else}
          <div class="mr-3">
            {#each state?.entries ?? [] as [key, value] (key)}
              <div class="flex items-center gap-3 py-1 border-b border-border text-[0.78rem]">
                <span class="flex-1 min-w-0 truncate text-text-bright">{key}</span>
                <span class="w-40 text-right text-text-muted truncate" title={value}>{value || "—"}</span>
                <button
                  class="shrink-0 text-text-muted hover:text-danger disabled:opacity-40"
                  title={$_("mangohud.remove")}
                  disabled={busy}
                  on:click={() => run(() => mangohudRemove(key))}
                >
                  <Trash2 size={13} />
                </button>
              </div>
            {/each}
          </div>
        {/if}

        {#if state}
          <p class="text-[0.7rem] text-text-muted mt-2">{$_("mangohud.keptLines", { values: { count: state.otherLines } })}</p>
        {/if}
      </div>
    {/if}
  </section>

  {#if showFile}
    <pre class="mt-3 max-h-[240px] overflow-auto inner-scroll rounded border border-border bg-dark/40 p-3 text-[0.72rem] text-text whitespace-pre">{file}</pre>
  {/if}
</div>
