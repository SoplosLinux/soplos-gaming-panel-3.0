<script lang="ts">
  import { createEventDispatcher } from "svelte";

  export let checked = false;
  export let disabled = false;
  export let label = "";

  const dispatch = createEventDispatcher<{ change: boolean }>();

  function toggle() {
    if (disabled) return;
    checked = !checked;
    dispatch("change", checked);
  }
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  class="relative w-10 h-[22px] rounded-full transition-colors shrink-0
    {checked ? 'bg-primary' : 'bg-surface-2 border border-border'} {disabled ? 'opacity-50 cursor-not-allowed' : ''}"
  on:click={toggle}
>
  <span class="absolute top-[3px] w-4 h-4 rounded-full bg-white transition-all {checked ? 'left-[21px]' : 'left-[3px]'}"></span>
</button>
