<script lang="ts">
  import HelpPill from "$lib/ui/HelpPill.svelte";
  import { stateTone, stateLabel, STATE_HINT } from "$lib/tone";

  // the one evidence-state pill (none, src-only, tagged test, …): same look and tooltip on every page
  let { state: evidence, deleted = false }: { state: string; deleted?: boolean } = $props();
  const title = $derived(`Evidence state: ${stateLabel(evidence)}${STATE_HINT[evidence] ? " — " + STATE_HINT[evidence] : ""}`);
</script>

<HelpPill label={stateLabel(evidence)} {title} tone={deleted ? "red" : stateTone[evidence]} />
