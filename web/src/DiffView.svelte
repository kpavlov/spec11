<script lang="ts">
  import { call } from "./transport";
  import Pill from "./Pill.svelte";
  import { changeTone } from "./tone";
  import { diffWrap, setDiffWrap } from "./layout.svelte";

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
  // old / new line numbers from the hunk headers (`@@ -a,b +c,d @@`): context counts on both sides, + on the new one, - on the old one
  const numbered = (diff: string) => {
    let o = 0, n = 0, inHunk = false;
    return diff.trimEnd().split("\n").map((t) => {
      const k = kind(t);
      const h = /^@@ -(\d+)(?:,\d+)? \+(\d+)/.exec(t);
      if (h) { [o, n, inHunk] = [+h[1], +h[2], true]; return { t, k }; }
      if (!inHunk || t[0] === "\\") return { t, k };
      if (t[0] === "+") return { t, k, n: n++ };
      if (t[0] === "-") return { t, k, o: o++ };
      return { t, k, o: o++, n: n++ };
    });
  };
</script>

{#each diffs as d (d.path)}
  <h4 class="mono nowrap">{#if href}<a href={href(d.path)}>{d.path}</a>{:else}{d.path}{/if} {#if d.change}<Pill tone={changeTone(d.change)}>{d.change}</Pill>{/if}{#if d.old_path} <span class="muted">from {d.old_path}</span>{/if} <label class="muted wrapctl"><input type="checkbox" checked={diffWrap.on} onchange={(e) => setDiffWrap(e.currentTarget.checked)} /> wrap long lines</label></h4>
  {#if d.error}<p class="muted">{d.error}</p>
  {:else if !d.diff.trim()}<p class="muted">No difference against HEAD.</p>
  {:else}<div class="diff" class:wrap={diffWrap.on}>{#each numbered(d.diff) as r}<div class="row {r.k}"><span class="gs"><i>{r.o ?? ""}</i><i>{r.n ?? ""}</i></span><span class="c">{r.t}</span></div>{#each reqs(d, r.t) as q}<div class="row touch"><span class="gs"><i></i><i></i></span><span class="c">↳ <b>{q.id}</b> {q.role} {q.action}</span></div>{/each}{/each}</div>{/if}
{/each}
{#if !diffs.length}<p class="muted">Loading the diff…</p>{/if}

<style>
  .wrapctl { font-weight: normal; margin-left: 8px; }
  .diff { overflow: auto; max-height: 70vh; max-width: 100%; padding: 8px 0; border: 1px solid var(--line); border-radius: var(--radius); background: var(--card); font: var(--text-label)/1.45 var(--mono); }
  .row { display: flex; width: max-content; min-width: 100%; }
  .wrap .row { width: auto; }
  .gs { display: flex; flex: none; position: sticky; left: 0; background: var(--card); color: var(--muted); font-size: var(--text-sm); user-select: none; border-right: 1px solid var(--line); }
  .gs i { font-style: normal; width: 5ch; padding: 0 6px; text-align: right; }
  .c { padding: 0 12px; white-space: pre; }
  .wrap .c { white-space: pre-wrap; overflow-wrap: anywhere; min-width: 0; flex: 1; }
  .add { background: color-mix(in srgb, var(--ok) 14%, transparent); }
  .add .gs { background: color-mix(in srgb, var(--ok) 14%, var(--card)); }
  .del { background: color-mix(in srgb, var(--bad) 14%, transparent); }
  .del .gs { background: color-mix(in srgb, var(--bad) 14%, var(--card)); }
  .hunk { color: var(--muted); background: color-mix(in srgb, var(--accent) 8%, transparent); }
  .hunk .gs { background: color-mix(in srgb, var(--accent) 8%, var(--card)); }
  .touch { color: var(--accent); font-size: var(--text-sm); }
  .meta { color: var(--muted); }
</style>
