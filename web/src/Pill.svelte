<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Tone } from "./tone";
  let { tone, title, children }: { tone?: Tone; title?: string; children: Snippet } = $props();
</script>

<span class="pill" data-tone={tone} {title}>{@render children()}</span>

<style>
  .pill { font-size: 11.5px; padding: 1px 8px; border-radius: 2px; margin-right: 4px; white-space: nowrap; color: var(--tone, inherit); background: color-mix(in srgb, currentColor 10%, transparent); }
</style>
