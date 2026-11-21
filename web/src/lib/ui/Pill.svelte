<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Tone } from "$lib/tone";
  let { tone, title, children }: { tone?: Tone; title?: string; children: Snippet } = $props();
</script>

<span class="pill" data-tone={tone} {title}>{@render children()}</span>

<style>
  .pill { display: inline-block; font-size: var(--badge-size); font-weight: var(--weight-medium); padding: var(--badge-padding); border-radius: var(--badge-radius); margin-right: 4px; white-space: nowrap; color: var(--pill-ink, var(--tone, inherit)); background: var(--pill-fill, color-mix(in srgb, currentColor 10%, var(--card))); }
</style>
