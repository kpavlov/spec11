<script lang="ts">
  import { layout, toggleSplit } from "./layout.svelte";
</script>

<button type="button" class="icon" onclick={toggleSplit} title={layout.split === "v" ? "List left, preview right (click: list on top)" : "List on top, preview below (click: list left)"} aria-label="Split layout: {layout.split === 'v' ? 'list left, preview right' : 'list on top, preview below'}">{layout.split === "v" ? "◫" : "⬒"}</button>
