<script lang="ts">
  import type { ComponentType } from "svelte";

  // The banner over every page: the image of the program, the icon of the
  // page, its title and the line that says what the page is for.
  export let title: string;
  export let subtitle = "";
  export let icon: ComponentType;
</script>

<div
  class="rounded px-5 py-8 mb-4 flex items-center gap-3.5 border border-border"
  style="background: linear-gradient(rgba(20, 20, 22, 0.55), rgba(20, 20, 22, 0.82)),
    url('/assets/hero/gaming-panel-background.jpg'); background-size: cover; background-position: center;"
>
  <div class="w-[42px] h-[42px] rounded-lg grid place-items-center bg-white/15 text-white shrink-0">
    <svelte:component this={icon} size={22} />
  </div>
  <div class="min-w-0">
    <h2 class="text-[1.3rem] font-bold text-white leading-tight">{title}</h2>
    {#if subtitle}
      <p class="text-[0.82rem] text-white/70 mt-0.5">{subtitle}</p>
    {/if}
  </div>
</div>
