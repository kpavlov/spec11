<script lang="ts">
  import HelpPill from "./HelpPill.svelte";
  import { requirementLevelTone, stateTone } from "./tone";

  let { level, state: evidence, label, hint, deleted = false }: {
    level: string; state: string; label: string; hint: string; deleted?: boolean;
  } = $props();
  const levels: Record<string, string> = { MUST: "Required", SHOULD: "Recommended", MAY: "Optional", SHALL: "Required" };
  const levelTitle = $derived(`Requirement level: ${level}${levels[level] ? " — " + levels[level] : ""}`);
  const stateTitle = $derived(`Evidence state: ${label}${hint ? " — " + hint : ""}`);
</script>

<div class="metadata" role="group" aria-label="Requirement metadata">
  {#if level}<HelpPill label={level.toLowerCase()} title={levelTitle} tone={requirementLevelTone[level] ?? "muted"} />{/if}
  <HelpPill {label} title={stateTitle} tone={deleted ? "red" : stateTone[evidence]} />
</div>

<style>
  .metadata { display: flex; gap: 4px; margin-top: 3px; --badge-padding: 1px 6px; }
</style>
