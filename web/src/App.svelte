<script lang="ts">
  import { call } from "./transport";
  import Markdown from "./Markdown.svelte";

  type Ref = { file: string; line: number; text: string };
  type Row = { id: string; state: string; file: string; line: number; level: string; text: string; mark: string; done: boolean; verdict: string; built: boolean; issues: string[]; tests: Ref[]; cites: Ref[]; graph: string[] };

  let tab = $state<"report" | "requirements" | "specs" | "overlap" | "uncovered" | "setup">("report");
  let setup = $state<{ status: any; suggestions: string[]; text: string } | null>(null);
  let busy = $state(false);
  let checks = $state<{ name: string; level: string; detail: string; hint: string | null }[]>([]);
  let specFiles = $state<string[] | null>(null);
  let specFile = $state("");
  let specMd = $state("");
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

  async function loadSetup() {
    const [r, d] = await Promise.all([call("spec_install_status"), call("spec_doctor")]);
    setup = { ...r.data, text: r.text };
    checks = d.data.checks;
  }

  async function runSetup(tool: "spec_install" | "spec_uninstall", global = false) {
    busy = true;
    try {
      const r = await guard(call(tool, { global }));
      setup = { ...r.data, text: r.text };
    } finally {
      busy = false;
    }
  }

  async function openSpec(name: string) {
    specFile = name;
    specMd = (await guard(call("spec_file", { name }))).data.markdown;
  }

  // an ID clicked anywhere (markdown, chips) opens its trace next to the requirement table
  function goto(id: string) {
    tab = "requirements";
    stateFilter = "all";
    query = id;
    open(id);
  }

  $effect(() => {
    if (tab === "specs" && specFiles === null)
      call("spec_file").then((r) => { specFiles = r.data.files; if (specFiles?.length) openSpec(specFiles[0]); }).catch((e) => (error = e.message));
    if (tab === "overlap" && overlap === null) call("spec_overlap").then((r) => (overlap = r.data.rows)).catch((e) => (error = e.message));
    if (tab === "uncovered" && uncovered === null) call("spec_uncovered").then((r) => (uncovered = r.data)).catch((e) => (error = e.message));
  });

  load();
  loadSetup().catch(() => {});

  const states = ["all", "tested", "src-only", "none", "manual", "later"];
  const shown = $derived(rows.filter((r) => (stateFilter === "all" || r.state === stateFilter) && (!query || `${r.id} ${r.text} ${r.file}`.toLowerCase().includes(query.toLowerCase()))));
  const problems = $derived(report ? Object.entries(report.problems as Record<string, number>) : []);
  const plain = (t: string) => t.replace(/[`*~]/g, "");
  const label = (k: string) => k.replaceAll("_", " ");
</script>

{#snippet tracePanel()}
  {#if trace}
    {@const sp = trace.data?.spec}
    <aside>
      <div class="ahead">
        <b class="mono nowrap">{trace.data.id}</b>
        {#if sp}<span class="pill">{sp.level}</span>{/if}
        <button class="x" onclick={() => (trace = null)} aria-label="close trace">✕</button>
      </div>
      {#if sp}
        <p class="muted mono nowrap">{sp.file}:{sp.line}</p>
        <Markdown source={sp.text} onid={goto} />
      {:else}<p class="muted">Not in any spec: cited only in tests or code.</p>{/if}
      {#if trace.data.tests?.length}
        <h4>Tests ({trace.data.tests.length})</h4>
        <ul class="refs">{#each trace.data.tests as t}<li><span class="mono nowrap muted">{t.file}:{t.line}</span> {t.text}</li>{/each}</ul>
      {/if}
      {#if trace.data.source_cites?.length}
        <h4>Code citations ({trace.data.source_cites.length})</h4>
        <ul class="refs">{#each trace.data.source_cites as c}<li><span class="mono nowrap muted">{c.file}:{c.line}</span> <code>{c.decl}</code></li>{/each}</ul>
      {/if}
      {#if trace.data.related?.length}
        <h4>Related code ({trace.data.related_total})</h4>
        <ul class="refs">{#each trace.data.related as r}<li><span class="mono nowrap">{r.file}</span> <span class="muted">{r.types.join(", ")}{r.cites ? " · cites it" : ""}</span></li>{/each}</ul>
      {/if}
      <details><summary class="muted">plain text</summary><pre>{trace.text}</pre></details>
      <p class="muted note">Evidence, not truth.</p>
    </aside>
  {/if}
{/snippet}

<header>
  <h1>spec11</h1>
  <span class="mono muted">{root}</span>
  <nav>
    {#each ["report", "requirements", "specs", "overlap", "uncovered", "setup"] as t}
      <button class:on={tab === t} aria-current={tab === t ? "page" : undefined} onclick={() => (tab = t as typeof tab)}>{t}</button>
    {/each}
    <button onclick={load} title="rescan" aria-label="rescan">↻</button>
  </nav>
</header>

{#if error}<p class="error">{error}</p>{/if}
{#if setup && setup.suggestions.length && tab !== "setup"}
  <p class="banner">
    <span>spec11 is not fully set up for this repo: {setup.suggestions[0]}</span>
    <span class="grow"></span>
    <button disabled={busy} onclick={() => runSetup("spec_install")}>Install here</button>
    <button onclick={() => (tab = "setup")}>Details</button>
  </p>
{/if}

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
      <details open={list.length > 0}>
        <summary>{title} <b>{list.length}</b></summary>
        <p class="ids">{#each list as id}<button class="id" onclick={() => goto(id)}>{id}</button>{/each}</p>
      </details>
    {/snippet}
    {@render ids("unimplemented: due, no test and no code", report.unimplemented)}
    {@render ids("untested: due, cited in code but no test", report.untested)}
    {@render ids("marked later but already has tests or code", report.built)}

    <details open={report.unproven.length > 0}>
      <summary>check-marked done without proof <b>{report.unproven.length}</b></summary>
      <ul>{#each report.unproven as u}<li><button class="id" onclick={() => open(u.id)}>{u.id}</button> <code>{u.verdict}</code> {u.file}:{u.line} <span class="muted">{plain(u.text).slice(0, 80)}</span></li>{/each}</ul>
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
      <select bind:value={stateFilter} name="state" aria-label="state">{#each states as s}<option>{s}</option>{/each}</select>
      <input name="filter" aria-label="filter" placeholder="filter ID, text, file" bind:value={query} />
      <span class="muted">{shown.length} of {rows.length}</span>
    </div>
    <div class="split">
      <table>
        <tbody>
          {#each shown.slice(0, 500) as r (r.id)}
            <tr onclick={() => open(r.id)} onkeydown={(e) => e.key === "Enter" && open(r.id)} tabindex="0" class:sel={trace?.data?.id === r.id}>
              <td class="mono nowrap">{r.id}</td>
              <td><span class="pill {r.state}">{r.state}</span>{#if r.done}<span class="pill done" title="check-marked: {r.verdict}">✓ {r.verdict}</span>{/if}</td>
              <td class="txt" title={plain(r.text)}>{plain(r.text)}</td>
              <td class="mono muted nowrap">{r.file}:{r.line}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      {@render tracePanel()}
    </div>
    {#if shown.length > 500}<p class="muted">First 500 shown; narrow the filter.</p>{/if}
  {:else if tab === "setup"}
    {#if !setup}<p class="muted">Checking…</p>
    {:else}
      {#snippet row(title: string, where: any, global: boolean)}
        <div class="card block setup">
          <h3>{title}</h3>
          <p><span class="pill {where.skill === 'current' ? 'tested' : where.skill === 'outdated' ? 'src-only' : 'none'}">skill {where.skill}</span> <span class="mono muted">{where.skill_path}</span></p>
          <p><span class="pill {where.mcp === 'ok' ? 'tested' : where.mcp === 'other' ? 'src-only' : 'none'}">mcp {where.mcp === "ok" ? "installed" : where.mcp === "other" ? "other port" : "missing"}</span> <span class="mono muted">{where.mcp_path ?? "~/.claude.json (user scope, read-only here)"}</span></p>
          <p>
            <button disabled={busy} onclick={() => runSetup("spec_install", global)}>{where.skill === "missing" ? "Install" : "Reinstall"}{global ? " skill globally" : " in this repo"}</button>
            <button disabled={busy || (where.skill === "missing" && where.mcp !== "ok")} onclick={() => runSetup("spec_uninstall", global)}>Uninstall</button>
          </p>
        </div>
      {/snippet}
      {@render row("This repo", setup.status.project, false)}
      {@render row("Global (~/.agents/skills)", setup.status.global, true)}
      <p class="muted">The skill tells the agent to use spec11's MCP tools. Installing in the repo also adds the MCP server to <code>.mcp.json</code> (other servers are kept); the global install covers the skill only. The agent needs this server running: <code>spec11</code>.</p>
      <h3>Checks <button onclick={loadSetup}>↻</button></h3>
      <table>
        <tbody>
          {#each checks as k}
            <tr>
              <td class="nowrap"><span class="pill {k.level === 'ok' ? 'tested' : k.level === 'warn' ? 'src-only' : 'none'}">{k.level}</span></td>
              <td class="nowrap">{k.name}</td>
              <td>{k.detail}{#if k.hint}<div class="muted">→ {k.hint}</div>{/if}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  {:else if tab === "specs"}
    {#if specFiles === null}<p class="muted">Loading…</p>
    {:else}
      <div class="specs">
        <ul class="files">{#each specFiles as f}<li><button class:on={f === specFile} onclick={() => openSpec(f)}>{f.replace(/\.md$/, "")}</button></li>{/each}</ul>
        <article><Markdown source={specMd} onid={goto} /></article>
      </div>
    {/if}
  {:else if tab === "overlap"}
    {#if overlap === null}<p class="muted">Loading…</p>
    {:else if overlap.length === 0}<p class="muted">No declaration is covered by two or more requirements (or no CodeGraph index).</p>
    {:else}
      {#each overlap as r}
        <div class="card block">
          <div class="mono nowrap">{r.file}:{r.line} <b>{r.name}</b></div>
          {#each r.pairs as [a, b, linked]}
            <div><button class="id" onclick={() => open(a)}>{a}</button> {linked ? "~ linked" : "? UNLINKED"} <button class="id" onclick={() => open(b)}>{b}</button></div>
          {/each}
        </div>
      {/each}
      {@render tracePanel()}
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
  .banner { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin: 0; padding: 8px 16px; background: color-mix(in srgb, var(--warn) 14%, var(--bg)); border-bottom: 1px solid var(--line); font-size: 13px; }
  .grow { flex: 1; }
  .setup h3 { margin: 0 0 6px; font-size: 15px; }
  .setup p { margin: 6px 0; }
  button:disabled { opacity: 0.5; cursor: default; }
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
  .id { font-family: var(--mono); font-size: 12px; padding: 1px 6px; white-space: nowrap; }
  .nowrap { white-space: nowrap; }
  .bar { display: flex; gap: 8px; align-items: center; margin-bottom: 12px; }
  .bar input { flex: 1; min-width: 160px; }
  .split { display: grid; grid-template-columns: minmax(0, 1fr); gap: 12px; }
  @media (min-width: 1000px) { .split:has(aside) { grid-template-columns: minmax(0, 3fr) minmax(0, 2fr); } }
  table { width: 100%; border-collapse: collapse; }
  .txt { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  td { padding: 4px 8px; border-bottom: 1px solid var(--line); vertical-align: top; }
  tr[onclick] { cursor: pointer; }
  tr.sel, tr[onclick]:hover { background: color-mix(in srgb, var(--accent) 10%, transparent); }
  aside { position: sticky; top: 8px; align-self: start; background: var(--card); border: 1px solid var(--line); border-radius: 8px; padding: 8px 12px; overflow: auto; max-height: 85vh; }
  header { position: sticky; top: 0; background: var(--bg); z-index: 1; }
  tr:focus-visible, button:focus-visible, summary:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .ahead { display: flex; align-items: center; gap: 8px; }
  .ahead .x { margin-left: auto; padding: 0 8px; }
  aside h4 { margin: 12px 0 4px; font-size: 13px; }
  .refs { margin: 0; padding-left: 16px; font-size: 12.5px; }
  .note { font-size: 12px; margin: 8px 0 0; }
  .specs { display: grid; grid-template-columns: 220px minmax(0, 1fr); gap: 16px; align-items: start; }
  .files { list-style: none; margin: 0; padding: 0; position: sticky; top: 56px; max-height: 80vh; overflow: auto; display: flex; flex-direction: column; gap: 2px; }
  .files button { width: 100%; text-align: left; border-color: transparent; background: none; }
  .files button.on { border-color: var(--accent); }
  article { background: var(--card); border: 1px solid var(--line); border-radius: 8px; padding: 4px 16px 12px; }
  @media (max-width: 800px) { .specs { grid-template-columns: 1fr; } .files { position: static; max-height: 30vh; } }
  @media (max-width: 999px) { aside { order: -1; position: static; max-height: none; } }
  pre { margin: 0; white-space: pre-wrap; word-break: break-word; }
  .pill { font-size: 11px; padding: 1px 6px; border-radius: 10px; border: 1px solid var(--line); margin-right: 4px; }
  .pill.tested { color: var(--ok); } .pill.none { color: var(--bad); } .pill.src-only { color: var(--warn); } .pill.done { color: var(--accent); }
</style>
