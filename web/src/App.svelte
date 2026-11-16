<script lang="ts">
  import { call } from "./transport";

  type Ref = { file: string; line: number; text: string };
  type Row = { id: string; state: string; file: string; line: number; level: string; text: string; mark: string; done: boolean; verdict: string; built: boolean; issues: string[]; tests: Ref[]; cites: Ref[]; graph: string[] };

  let tab = $state<"report" | "requirements" | "overlap" | "uncovered">("report");
  let report = $state<any>(null);
  let rows = $state<Row[]>([]);
  let overlap = $state<any[] | null>(null);
  let uncovered = $state<any>(null);
  let trace = $state<{ text: string; data: any } | null>(null);
  let root = $state("");
  let error = $state("");
  let stateFilter = $state("all");
  let query = $state("");

  const guard = <T,>(p: Promise<T>) => p.catch((e) => { error = String(e.message ?? e); throw e; });

  async function load() {
    error = "";
    const [r, s] = await guard(Promise.all([call("spec_report", { limit: 1000 }), call("spec_status", { all: true, limit: 5000 })]));
    report = r.data;
    rows = s.data.rows;
    overlap = uncovered = null;
    trace = null;
    if (!root) fetch("/api/info").then((x) => x.json()).then((i) => (root = i.root ?? "")).catch(() => {});
  }

  async function open(id: string) {
    trace = await guard(call("spec_trace", { id }));
  }

  $effect(() => {
    if (tab === "overlap" && overlap === null) call("spec_overlap").then((r) => (overlap = r.data.rows)).catch((e) => (error = e.message));
    if (tab === "uncovered" && uncovered === null) call("spec_uncovered").then((r) => (uncovered = r.data)).catch((e) => (error = e.message));
  });

  load();

  const states = ["all", "tested", "src-only", "none", "manual", "later"];
  const shown = $derived(rows.filter((r) => (stateFilter === "all" || r.state === stateFilter) && (!query || `${r.id} ${r.text} ${r.file}`.toLowerCase().includes(query.toLowerCase()))));
  const problems = $derived(report ? Object.entries(report.problems as Record<string, number>) : []);
  const label = (k: string) => k.replaceAll("_", " ");
</script>

<header>
  <h1>spec11</h1>
  <span class="mono muted">{root}</span>
  <nav>
    {#each ["report", "requirements", "overlap", "uncovered"] as t}
      <button class:on={tab === t} onclick={() => (tab = t as typeof tab)}>{t}</button>
    {/each}
    <button onclick={load} title="rescan">↻</button>
  </nav>
</header>

{#if error}<p class="error">{error}</p>{/if}

<main>
  {#if !report}
    <p class="muted">Scanning…</p>
  {:else if tab === "report"}
    <section class="cards">
      <div class="card"><b>{report.tested_pct}%</b><span>tested ({report.tested}/{report.due} due)</span></div>
      <div class="card"><b>{report.requirements}</b><span>requirements in {report.spec_files} specs</span></div>
      <div class="card"><b>{report.manual}</b><span>manual</span></div>
      <div class="card"><b>{report.later}</b><span>later</span></div>
      <div class="card" class:bad={report.total_problems > 0}><b>{report.total_problems}</b><span>problems</span></div>
    </section>
    {#if !report.index}<p class="muted">No CodeGraph index: test-to-code links and uncovered code are not checked. {report.index_note ?? ""}</p>{/if}
    <section class="tally">
      {#each problems as [k, v]}<span class:zero={v === 0}>{label(k)} <b>{v}</b></span>{/each}
    </section>

    {#snippet ids(title: string, list: string[])}
      <details open={list.length > 0 && list.length < 30}>
        <summary>{title} <b>{list.length}</b></summary>
        <p class="ids">{#each list as id}<button class="id" onclick={() => { tab = "requirements"; query = id; open(id); }}>{id}</button>{/each}</p>
      </details>
    {/snippet}
    {@render ids("unimplemented: due, no test and no code", report.unimplemented)}
    {@render ids("untested: due, cited in code but no test", report.untested)}
    {@render ids("marked later but already has tests or code", report.built)}

    <details open={report.unproven.length > 0}>
      <summary>check-marked done without proof <b>{report.unproven.length}</b></summary>
      <ul>{#each report.unproven as u}<li><button class="id" onclick={() => open(u.id)}>{u.id}</button> <code>{u.verdict}</code> {u.file}:{u.line} <span class="muted">{u.text.slice(0, 80)}</span></li>{/each}</ul>
    </details>
    <details open={Object.values(report.invalid_tests).some((v: any) => v.length)}>
      <summary>invalid tests <b>{Object.values(report.invalid_tests).reduce((n: number, v: any) => n + v.length, 0)}</b></summary>
      {#each Object.entries(report.invalid_tests) as [kind, list]}
        {#if (list as any[]).length}<p><code>{kind}</code> {#each list as any[] as p}<button class="id" onclick={() => open(p.id)}>{p.id}</button> <span class="muted mono">{p.file}:{p.line}</span> {/each}</p>{/if}
      {/each}
    </details>
    <details open={report.spec_issues.length > 0}>
      <summary>spec issues <b>{report.spec_issues.length}</b></summary>
      <ul>{#each report.spec_issues as x}<li class="mono">{x}</li>{/each}</ul>
    </details>
    <details>
      <summary>dangling citations <b>{report.dangling_tests.length + report.dangling_sources.length}</b></summary>
      <p class="mono">tests: {report.dangling_tests.join(", ") || "none"}<br />sources: {report.dangling_sources.join(", ") || "none"}</p>
    </details>
  {:else if tab === "requirements"}
    <div class="bar">
      <select bind:value={stateFilter}>{#each states as s}<option>{s}</option>{/each}</select>
      <input placeholder="filter ID, text, file" bind:value={query} />
      <span class="muted">{shown.length} of {rows.length}</span>
    </div>
    <div class="split">
      <table>
        <tbody>
          {#each shown.slice(0, 500) as r (r.id)}
            <tr onclick={() => open(r.id)} class:sel={trace?.data?.id === r.id}>
              <td class="mono">{r.id}</td>
              <td><span class="pill {r.state}">{r.state}</span>{#if r.done}<span class="pill done" title="check-marked: {r.verdict}">✓ {r.verdict}</span>{/if}</td>
              <td>{r.text.slice(0, 110)}</td>
              <td class="mono muted">{r.file}:{r.line}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if trace}
        <aside><pre>{trace.text}</pre></aside>
      {/if}
    </div>
    {#if shown.length > 500}<p class="muted">First 500 shown; narrow the filter.</p>{/if}
  {:else if tab === "overlap"}
    {#if overlap === null}<p class="muted">Loading…</p>
    {:else if overlap.length === 0}<p class="muted">No declaration is covered by two or more requirements (or no CodeGraph index).</p>
    {:else}
      {#each overlap as r}
        <div class="card block">
          <div class="mono">{r.file}:{r.line} <b>{r.name}</b></div>
          {#each r.pairs as [a, b, linked]}
            <div><button class="id" onclick={() => open(a)}>{a}</button> {linked ? "~ linked" : "? UNLINKED"} <button class="id" onclick={() => open(b)}>{b}</button></div>
          {/each}
        </div>
      {/each}
      {#if trace}<pre>{trace.text}</pre>{/if}
    {/if}
  {:else if tab === "uncovered"}
    {#if uncovered === null}<p class="muted">Loading…</p>
    {:else if !uncovered.files}<p class="muted">Not available (no CodeGraph index).</p>
    {:else}
      <p>{uncovered.units} units / {uncovered.lines} lines in {uncovered.files.length} files have no requirement. Candidates, not proof.</p>
      <table>
        <tbody>
          {#each uncovered.files as f}
            <tr><td class="mono">{f.file}</td><td>{f.lines} lines</td><td class="mono muted">{f.units.map((u: any) => `${u.name}:${u.lo}-${u.hi}`).join(", ")}</td></tr>
          {/each}
        </tbody>
      </table>
    {/if}
  {/if}
</main>

<footer class="muted">Evidence, not truth: a citation or tag does not prove a requirement is implemented, nor that its test passes.</footer>

<style>
  header { display: flex; align-items: baseline; gap: 12px; padding: 12px 16px; border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  h1 { margin: 0; font-size: 18px; }
  nav { margin-left: auto; display: flex; gap: 6px; }
  main { padding: 16px; max-width: 1200px; margin: 0 auto; }
  footer { padding: 16px; text-align: center; font-size: 12px; }
  .muted { color: var(--muted); }
  .error { margin: 8px 16px; padding: 8px 12px; border: 1px solid var(--bad); color: var(--bad); border-radius: 6px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 12px; margin-bottom: 12px; }
  .card { background: var(--card); border: 1px solid var(--line); border-radius: 8px; padding: 12px; display: flex; flex-direction: column; }
  .card b { font-size: 24px; }
  .card.bad b { color: var(--bad); }
  .card.block { margin-bottom: 8px; }
  .tally { display: flex; flex-wrap: wrap; gap: 8px 16px; margin: 12px 0; font-size: 13px; }
  .tally .zero { color: var(--muted); }
  details { background: var(--card); border: 1px solid var(--line); border-radius: 8px; padding: 8px 12px; margin-bottom: 8px; }
  summary { cursor: pointer; }
  .ids { display: flex; flex-wrap: wrap; gap: 4px; }
  .id { font-family: var(--mono); font-size: 12px; padding: 1px 6px; }
  .bar { display: flex; gap: 8px; align-items: center; margin-bottom: 12px; }
  .bar input { flex: 1; min-width: 160px; }
  .split { display: grid; grid-template-columns: minmax(0, 1fr); gap: 12px; }
  @media (min-width: 1000px) { .split:has(aside) { grid-template-columns: minmax(0, 3fr) minmax(0, 2fr); } }
  table { width: 100%; border-collapse: collapse; }
  td { padding: 4px 8px; border-bottom: 1px solid var(--line); vertical-align: top; }
  tr[onclick] { cursor: pointer; }
  tr.sel, tr[onclick]:hover { background: color-mix(in srgb, var(--accent) 10%, transparent); }
  aside { position: sticky; top: 8px; align-self: start; background: var(--card); border: 1px solid var(--line); border-radius: 8px; padding: 8px 12px; overflow: auto; max-height: 85vh; }
  pre { margin: 0; white-space: pre-wrap; word-break: break-word; }
  .pill { font-size: 11px; padding: 1px 6px; border-radius: 10px; border: 1px solid var(--line); margin-right: 4px; }
  .pill.tested { color: var(--ok); } .pill.none { color: var(--bad); } .pill.src-only { color: var(--warn); } .pill.done { color: var(--accent); }
</style>
