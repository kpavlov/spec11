<script lang="ts">
  import { render } from "./markdown";

  let { source }: { source: string } = $props();
  const html = $derived(render(source));
</script>

<div class="md">{@html html}</div>

<style>
  .md { line-height: 1.55; overflow-wrap: anywhere; }
  .md :global(h1) { font-size: var(--text-title); margin: 20px 0 8px; }
  .md :global(h2) { font-size: var(--text-section); margin: 18px 0 6px; padding-bottom: 4px; border-bottom: 1px solid var(--line); }
  .md :global(h3), .md :global(h4), .md :global(h5), .md :global(h6) { font-size: var(--text-body); margin: 14px 0 4px; }
  .md :global(h5), .md :global(h6), .md :global(strong) { font-weight: var(--weight-semibold); }
  .md :global(p) { margin: 6px 0; }
  .md :global(ul) { margin: 6px 0; padding-left: 20px; }
  .md :global(li) { margin: 3px 0; }
  .md :global(code) { font-family: var(--mono); font-size: var(--text-label); background: color-mix(in srgb, var(--fg) 8%, transparent); padding: 1px 4px; border-radius: var(--radius); }
  .md :global(pre) { background: color-mix(in srgb, var(--fg) 6%, transparent); padding: 8px 10px; border-radius: 6px; overflow: auto; }
  .md :global(pre code) { background: none; padding: 0; }
  .md :global(table) { border-collapse: collapse; margin: 8px 0; display: block; overflow-x: auto; }
  .md :global(th), .md :global(td) { border: 1px solid var(--line); padding: 3px 8px; text-align: left; }
  .md :global(a) { color: var(--accent); }
  .md :global(a.id) { font-family: var(--mono); font-size: var(--text-sm); font-weight: var(--weight-semibold); white-space: nowrap; text-decoration: none; border-bottom: 1px dotted currentColor; }
</style>
