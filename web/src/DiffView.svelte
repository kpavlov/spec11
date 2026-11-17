<script lang="ts">
  import { call } from "./transport";
  import Pill from "./Pill.svelte";
  import { changeTone } from "./tone";

  // the unified diff against HEAD of each path (the `diff` tool), refetched when `gen` moves (the repo changed)
  let { paths, gen, href, scope = {} }: { paths: string[]; gen: number; href?: (path: string) => string; scope?: Record<string, unknown> } = $props();
  type D = { path: string; change: string; diff: string; old_path?: string | null; hunks: { header: string; requirements: { id: string; role: string; action: string }[] }[]; error?: string };
  let diffs = $state<D[]>([]);
  let asked = 0;
  $effect(() => {
    void gen;
    const ps = [...paths];
    const me = ++asked;
    Promise.all(ps.map((path) => call("diff", { ...scope, path }).then((r): D => ({ path, change: r.data.change, diff: r.data.diff, old_path: r.data.old_path, hunks: r.data.hunks ?? [] })).catch((e): D => ({ path, change: "", diff: "", hunks: [], error: String(e.message ?? e) })))).then((d) => { if (me === asked) diffs = d; });
  });
  const reqs = (d: D, l: string) => d.hunks.find((h) => h.header === l)?.requirements ?? [];
  const kind = (l: string) => (/^(\+\+\+|---|diff |index |new file|deleted file|similarity|rename )/.test(l) ? "meta" : l.startsWith("@@") ? "hunk" : l.startsWith("+") ? "add" : l.startsWith("-") ? "del" : "");
</script>

{#each diffs as d (d.path)}
  <h4 class="mono nowrap">{#if href}<a href={href(d.path)}>{d.path}</a>{:else}{d.path}{/if} {#if d.change}<Pill tone={changeTone(d.change)}>{d.change}</Pill>{/if}{#if d.old_path} <span class="muted">from {d.old_path}</span>{/if}</h4>
  {#if d.error}<p class="muted">{d.error}</p>
  {:else if !d.diff.trim()}<p class="muted">No difference against HEAD.</p>
  {:else}<pre class="diff">{#each d.diff.trimEnd().split("\n") as l}<span class={kind(l)}>{l}</span>{#each reqs(d, l) as r}<span class="touch">↳ <b>{r.id}</b> {r.role} {r.action}</span>{/each}{/each}</pre>{/if}
{/each}
{#if !diffs.length}<p class="muted">Loading the diff…</p>{/if}

<style>
  .diff { overflow-x: auto; padding: 8px 0; border: 1px solid var(--line); border-radius: var(--radius); background: var(--card); line-height: 1.45; }
  .diff span { display: block; padding: 0 12px; white-space: pre; }
  .add { background: color-mix(in srgb, var(--ok) 14%, transparent); }
  .del { background: color-mix(in srgb, var(--bad) 14%, transparent); }
  .hunk { color: var(--muted); background: color-mix(in srgb, var(--accent) 8%, transparent); }
  .touch { color: var(--accent); font-size: 0.9em; }
  .meta { color: var(--muted); }
</style>
