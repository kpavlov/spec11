<script lang="ts">
  import type { Snippet } from "svelte";
  import { MediaQuery } from "svelte/reactivity";

  let { active, children }: { active: number; children: Snippet } = $props();
  const narrow = new MediaQuery("(max-width: 48rem)");
</script>

<!-- (WEB-43) -->
{#if narrow.current}
  <details>
    <summary>Filters{active ? ` (${active})` : ""}</summary>
    <div>{@render children()}</div>
  </details>
{:else}
  {@render children()}
{/if}

<style>
  details { max-width: 100%; }
  details[open] { width: 100%; }
  summary { cursor: pointer; color: var(--accent); }
  div { display: flex; flex-wrap: wrap; align-items: center; gap: var(--gap); padding-block: var(--gap); }
</style>
