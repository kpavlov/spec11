<script lang="ts">
  import { untrack, type Snippet } from "svelte";
  import type { Tone } from "$lib/tone";
  import { loadSection, saveSection } from "$lib/layout/layout.svelte";
  // a collapsible block with a title and a count; its tone colours the left edge, the tint and the count.
  // With an `id` (WEB-34) it also remembers per browser whether it is open and the height dragged on its bottom edge.
  let { title, count, tone, open = true, id, children }: { title: string; count?: number; tone?: Tone; open?: boolean; id?: string; children: Snippet } = $props();
  // `id` and the default are read once: what was remembered takes over from there
  const kept = untrack(() => (id ? loadSection(id) : null));
  let isOpen = $state(kept ? kept.open : untrack(() => open));
  let height = $state<number | null>(kept?.h ?? null);
  let body = $state<HTMLElement>();
  $effect(() => { if (id) saveSection(id, { open: isOpen, h: height }); });
  // the browser's resize handle sets an inline height; take it over from there
  $effect(() => {
    if (!body) return;
    const ro = new ResizeObserver(() => { const h = Math.round(parseFloat(body!.style.height)); if (h > 0) height = h; });
    ro.observe(body);
    return () => ro.disconnect();
  });
  const auto = (e: Event) => { e.preventDefault(); height = null; if (body) body.style.height = ""; };
</script>

<details class="section" data-tone={tone} bind:open={isOpen}>
  <summary><span>{title}</span>{#if height !== null}<button class="auto" title="Back to the automatic height" aria-label="Back to the automatic height of {title}" onclick={auto}>↕</button>{/if}{#if count !== undefined}<b class="count">{count}</b>{/if}</summary>
  {#if id}
    <div class="body" bind:this={body} style:height={height === null ? null : `${height}px`}>{@render children()}</div>
  {:else}
    {@render children()}
  {/if}
</details>

<style>
  .section { background: var(--card); border: 1px solid var(--line); border-left: 4px solid var(--line); border-radius: var(--radius); padding: 10px 14px; margin-bottom: var(--gap); }
  .section[data-tone] { border-left-color: var(--tone); background: color-mix(in srgb, var(--tone) 5%, var(--card)); }
  summary { cursor: pointer; display: flex; align-items: center; gap: 8px; }
  summary > span { font-size: var(--text-section); line-height: 1.3; font-weight: var(--weight-semibold); }
  summary span { flex: 1; }
  .section > :global(:not(summary)) { margin-top: var(--gap); }
  .body { overflow: auto; resize: vertical; min-height: 3em; }
  .body > :global(* + *) { margin-top: var(--gap); }
  .auto { all: unset; cursor: pointer; font-weight: 400; color: var(--muted); padding: 0 6px; }
  .auto:hover, .auto:focus-visible { color: var(--fg); }
  .section :global(ul) { margin: 0; padding-left: 18px; }
  .section :global(li) { margin: 4px 0; }
  .count { font-size: var(--text-sm); padding: 0 7px; border-radius: 2px; color: var(--card); background: var(--tone, var(--muted)); }
</style>
