<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Tone } from "./tone";
  // a collapsible block with a title and a count; its tone colours the left edge, the tint and the count
  let { title, count, tone, open = true, children }: { title: string; count?: number; tone?: Tone; open?: boolean; children: Snippet } = $props();
</script>

<details class="section" data-tone={tone} {open}>
  <summary><span>{title}</span>{#if count !== undefined}<b class="count">{count}</b>{/if}</summary>
  {@render children()}
</details>

<style>
  .section { background: var(--card); border: 1px solid var(--line); border-left: 4px solid var(--line); border-radius: var(--radius); padding: 10px 14px; margin-bottom: var(--gap); }
  .section[data-tone] { border-left-color: var(--tone); background: color-mix(in srgb, var(--tone) 5%, var(--card)); }
  summary { cursor: pointer; display: flex; align-items: center; gap: 8px; font-weight: 600; }
  summary span { flex: 1; }
  .section > :global(:not(summary)) { margin-top: var(--gap); }
  .section :global(ul) { margin: 0; padding-left: 18px; }
  .section :global(li) { margin: 4px 0; }
  .count { font-size: var(--text-xs); padding: 0 7px; border-radius: 2px; color: var(--card); background: var(--tone, var(--muted)); }
</style>
