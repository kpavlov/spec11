<script lang="ts">
  import HelpPill from "$lib/ui/HelpPill.svelte";
  import { requirementLevelTone } from "$lib/tone";

  // the one requirement-level pill (MUST, SHOULD, …): same look and tooltip on every page
  let { level }: { level: string } = $props();
  const meaning: Record<string, string> = { MUST: "Required", SHOULD: "Recommended", MAY: "Optional", SHALL: "Required" };
  const title = $derived(`Requirement level: ${level}${meaning[level] ? " — " + meaning[level] : ""}`);
</script>

<HelpPill label={level.toLowerCase()} {title} tone={requirementLevelTone[level] ?? "muted"} />
