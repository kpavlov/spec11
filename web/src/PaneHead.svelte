<script lang="ts">
  import type { Snippet } from "svelte";
  // the title row of a preview pane: the name, optional extras (a pill), and a close link that clears the selection
  let { title, close, children }: { title: string; close: string; children?: Snippet } = $props();
</script>

<div class="head">
  <h2 class="mono title">{title}</h2>
  {@render children?.()}
  <a class="x" href={close} aria-label="close preview">✕</a>
</div>

<style>
  .head { display: flex; align-items: baseline; gap: 8px; }
  .title { margin: 0; min-width: 0; font-size: var(--text-title); overflow-wrap: anywhere; }
  .x { margin-left: auto; text-decoration: none; padding: 0 8px; color: var(--muted); }
</style>
