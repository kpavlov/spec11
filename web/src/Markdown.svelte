<script lang="ts">
  import { render } from "./markdown";

  let { source, onid }: { source: string; onid?: (id: string) => void } = $props();
  const html = $derived(render(source));

  function click(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest<HTMLElement>("a[data-id]");
    if (!a || !onid) return;
    e.preventDefault();
    onid(a.dataset.id!);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="md" onclick={click}>{@html html}</div>

<style>
  .md { line-height: 1.55; overflow-wrap: anywhere; }
  .md :global(h1) { font-size: 20px; margin: 20px 0 8px; }
  .md :global(h2) { font-size: 17px; margin: 18px 0 6px; padding-bottom: 4px; border-bottom: 1px solid var(--line); }
  .md :global(h3), .md :global(h4) { font-size: 15px; margin: 14px 0 4px; }
  .md :global(p) { margin: 6px 0; }
  .md :global(ul) { margin: 6px 0; padding-left: 20px; }
  .md :global(li) { margin: 3px 0; }
  .md :global(code) { font-family: var(--mono); font-size: 12.5px; background: color-mix(in srgb, var(--fg) 8%, transparent); padding: 1px 4px; border-radius: 4px; }
  .md :global(pre) { background: color-mix(in srgb, var(--fg) 6%, transparent); padding: 8px 10px; border-radius: 6px; overflow: auto; }
  .md :global(pre code) { background: none; padding: 0; }
  .md :global(table) { border-collapse: collapse; margin: 8px 0; display: block; overflow-x: auto; }
  .md :global(th), .md :global(td) { border: 1px solid var(--line); padding: 3px 8px; text-align: left; }
  .md :global(a) { color: var(--accent); }
  .md :global(a.id) { font-family: var(--mono); font-size: 12.5px; white-space: nowrap; text-decoration: none; border-bottom: 1px dotted currentColor; }
</style>
