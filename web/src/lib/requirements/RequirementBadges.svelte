<script lang="ts">
  import LevelPill from "./LevelPill.svelte";
  import StatePill from "./StatePill.svelte";

  // level and evidence state of one requirement; the same pair on every page that shows it
  let { level, state: evidence, deleted = false }: { level?: string; state?: string; deleted?: boolean } = $props();
</script>

<div class="metadata" role="group" aria-label="Requirement metadata">
  {#if level}<LevelPill {level} />{/if}
  {#if evidence}<StatePill state={evidence} {deleted} />{/if}
</div>

<style>
  .metadata { display: inline-flex; gap: 4px; vertical-align: middle; --badge-padding: 1px 6px; }
</style>
