<script lang="ts">
  import type { Snippet } from "svelte";
  let { href, children, describedby }: { href: string; children: Snippet; describedby?: string } = $props();
</script>

<a class="id" {href} aria-describedby={describedby}>{@render children()}</a>

<style>
  a { font-family: var(--mono); font-size: var(--text-sm); padding: 1px 7px; white-space: nowrap; text-decoration: none; border: 1px solid var(--line); border-radius: 6px; background: var(--card); }
  a:hover { border-color: var(--accent); background: var(--tint-id); text-decoration: none; }
</style>
