<script lang="ts">
  import { setContext, onMount, type ComponentType } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getVersion } from "@tauri-apps/api/app";
  import { invoke } from "@tauri-apps/api/core";
  import { _ } from "svelte-i18n";
  import { listen } from "@tauri-apps/api/event";
  import { Activity, Zap, Gauge, Gamepad2, SlidersHorizontal } from "lucide-svelte";
  import { busy } from "./lib/progress";
  import Sidebar from "./components/Sidebar.svelte";
  import PageHero from "./components/PageHero.svelte";
  import WindowControls from "./components/WindowControls.svelte";
  import AboutDialog from "./components/AboutDialog.svelte";
  import Monitor from "./views/Monitor.svelte";
  import GameMode from "./views/GameMode.svelte";
  import MangoHud from "./views/MangoHud.svelte";
  import Library from "./views/Library.svelte";
  import Settings from "./views/Settings.svelte";

  // The tabs of the sidebar, in the order of the plan of the program. The
  // banner of each page takes the title, which is longer than the label.
  const tabDefs = [
    { id: "monitor", icon: Activity },
    { id: "gamemode", icon: Zap },
    { id: "mangohud", icon: Gauge },
    { id: "library", icon: Gamepad2 },
    { id: "settings", icon: SlidersHorizontal },
  ];

  $: tabs = tabDefs.map((tab) => ({
    ...tab,
    label: $_(`app.tabs.${tab.id}`),
    title: $_(`${tab.id}.title`),
    subtitle: $_(`${tab.id}.subtitle`),
  }));

  const views: Record<string, ComponentType> = {
    monitor: Monitor,
    gamemode: GameMode,
    mangohud: MangoHud,
    library: Library,
    settings: Settings,
  };

  let active = "monitor";
  let appVersion = "";
  let envInfo: { desktopEnvironment: string; displayProtocol: string } | null = null;
  let aboutOpen = false;

  // With `decorations` off in tauri.conf.json (and `transparent` on) the system
  // draws no frame: the window is one card with rounded corners and its
  // buttons float at the top right. With `decorations` on none of this shows,
  // so that setting is the only switch between the two.
  let frameless = false;
  let maximized = false;

  onMount(() => {
    let unlisten: (() => void) | undefined;
    let unlistenShow: (() => void) | undefined;
    (async () => {
      try {
        appVersion = await getVersion();
      } catch {
        appVersion = "";
      }
      try {
        envInfo = await invoke("get_environment_info");
      } catch {
        envInfo = null;
      }
      try {
        // The tray and the shortcut ask for a page of the window.
        unlistenShow = await listen<string>("show-view", (event) => select(event.payload));
      } catch {
        // Not running inside the program: nothing to listen to.
      }
      try {
        const win = getCurrentWindow();
        frameless = !(await win.isDecorated());
        if (frameless) {
          maximized = await win.isMaximized();
          unlisten = await win.onResized(async () => {
            maximized = await win.isMaximized();
          });
        }
      } catch {
        frameless = false;
      }
    })();
    return () => {
      unlisten?.();
      unlistenShow?.();
    };
  });

  function select(id: string) {
    if (!views[id]) return;
    active = id;
    document.getElementById("main-scroll")?.scrollTo({ top: 0 });
  }

  setContext("openAbout", () => (aboutOpen = true));

  $: ActiveView = views[active];
  $: activeTab = tabs.find((tab) => tab.id === active) ?? tabs[0];

  // F1 opens About, Ctrl+Q quits, Ctrl+Tab and Ctrl+Shift+Tab move between tabs.
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "F1") {
      e.preventDefault();
      aboutOpen = true;
    } else if (e.ctrlKey && (e.key === "q" || e.key === "Q")) {
      e.preventDefault();
      getCurrentWindow().close();
    } else if (e.ctrlKey && e.key === "Tab") {
      e.preventDefault();
      const i = tabs.findIndex((t) => t.id === active);
      const next = (i + (e.shiftKey ? -1 : 1) + tabs.length) % tabs.length;
      select(tabs[next].id);
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="relative flex h-screen overflow-hidden bg-dark {frameless && !maximized ? 'window-card' : ''}">
  <Sidebar {tabs} {active} onSelect={select} {appVersion} {envInfo} />

  {#if frameless}
    <div class="absolute right-2 top-[5px] z-40"><WindowControls {maximized} /></div>
  {/if}

  <div class="flex-1 flex flex-col overflow-hidden min-w-0">
    <!-- No page has a top bar: the strip over the content holds the window
         buttons and moves the window. -->
    {#if frameless}
      <div class="h-7 shrink-0" data-tauri-drag-region></div>
    {/if}
    <!-- The banner of the page stays where it is while the page scrolls under it. -->
    <div class="shrink-0 px-7 {frameless ? '' : 'pt-7'}">
      <PageHero title={activeTab.title} subtitle={activeTab.subtitle} icon={activeTab.icon} />
    </div>
    <div id="main-scroll" class="content-scroll flex-1 overflow-y-auto px-7">
      <svelte:component this={ActiveView} />
    </div>

    <!-- The bar of progress: it is there while the program works on something. -->
    <div class="h-1.5 bg-surface-2 overflow-hidden shrink-0" class:invisible={$busy === 0}>
      <div class="h-full w-1/3 bg-primary progress-slide"></div>
    </div>
  </div>

  {#if aboutOpen}
    <AboutDialog onClose={() => (aboutOpen = false)} />
  {/if}
</div>
