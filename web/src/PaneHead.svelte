<script lang="ts">
  import type { Snippet } from "svelte";
  // the title row of a preview pane: the name, optional extras (a pill), and a close link that clears the selection
  let { title, close, children }: { title: string; close: string; children?: Snippet } = $props();
</script>

<div class="head">
  <b class="mono title">{title}</b>
  {@render children?.()}
  <a class="x" href={close} aria-label="close preview">✕</a>
</div>

<style>
  .head { display: flex; align-items: center; gap: 8px; }
  .title { overflow-wrap: anywhere; }
  .x { margin-left: auto; text-decoration: none; padding: 0 8px; color: var(--muted); }
</style>
