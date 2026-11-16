<script lang="ts">
  import { untrack } from "svelte";
  import { call, embedded } from "./transport";
  import { TABS, format, parse, type Route } from "./router";
  import Markdown from "./Markdown.svelte";

  type Ref = { file: string; line: number; text: string };
  type Row = { id: string; state: string; file: string; line: number; level: string; text: string; mark: string; done: boolean; verdict: string; built: boolean; issues: string[]; tests: Ref[]; cites: Ref[]; graph: string[] };

  // ---- routing: the URL is the state ----
  const here = (): Route => {
    try { return parse(new URL(location.href)); } catch { return { tab: "report" }; }
  };
  let route = $state<Route>(here());
  const u = format;

  function navigate(r: Route, replace = false) {
    route = r;
    if (embedded) return; // a host iframe has no URL of its own
    try { (replace ? history.replaceState : history.pushState).call(history, null, "", format(r)); } catch { /* sandboxed */ }
  }

  $effect(() => {
    const onpop = () => (route = here());
    // same-origin links navigate in place; modified clicks keep the browser's own behaviour (new tab, download)
    const onclick = (e: MouseEvent) => {
      const a = (e.target as HTMLElement).closest<HTMLAnchorElement>("a[href^='/']");
      if (!a || e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || (a.target && a.target !== "_self")) return;
      e.preventDefault();
      navigate(parse(new URL(a.href)));
    };
    window.addEventListener("popstate", onpop);
    document.addEventListener("click", onclick);
    return () => { window.removeEventListener("popstate", onpop); document.removeEventListener("click", onclick); };
  });

  $effect(() => {
    document.title = ["spec11", route.tab, route.id ?? route.file ?? route.path].filter(Boolean).join(" · ");
  });

  // ---- theme: auto (OS) | light | dark, remembered per browser ----
  type Theme = "auto" | "light" | "dark";
  const themes: { v: Theme; icon: string; tip: string }[] = [{ v: "auto", icon: "◐", tip: "Follow the system theme" }, { v: "light", icon: "☀", tip: "Light theme" }, { v: "dark", icon: "☾", tip: "Dark theme" }];
  const saved = (): Theme => { try { const t = localStorage.getItem("spec11.theme"); return t === "light" || t === "dark" ? t : "auto"; } catch { return "auto"; } };
  let theme = $state<Theme>(saved());
  $effect(() => {
    const el = document.documentElement;
    if (theme === "auto") delete el.dataset.theme; else el.dataset.theme = theme;
    try { theme === "auto" ? localStorage.removeItem("spec11.theme") : localStorage.setItem("spec11.theme", theme); } catch { /* storage blocked */ }
  });

  // keyboard: `/` focuses the filter (opening the requirements list first), Esc closes the trace panel
  $effect(() => {
    const onkey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (t.closest("input, select, textarea") || e.metaKey || e.ctrlKey || e.altKey) { if (e.key === "Escape") (t as HTMLElement).blur(); return; }
      if (e.key === "/") {
        e.preventDefault();
        if (route.tab !== "requirements") navigate({ tab: "requirements" });
        queueMicrotask(() => document.querySelector<HTMLInputElement>("input[name=filter]")?.focus());
      } else if (e.key === "Escape" && route.id && (route.tab === "requirements" || route.tab === "overlap")) navigate({ ...route, id: undefined });
    };
    document.addEventListener("keydown", onkey);
    return () => document.removeEventListener("keydown", onkey);
  });

  // ---- data ----
  let setup = $state<{ status: any; suggestions: string[]; text: string } | null>(null);
  let busy = $state(false);
  let checks = $state<{ name: string; level: string; detail: string; hint: string | null }[]>([]);
  let indexAction = $state<"init" | "sync" | null>(null);
  let indexLog = $state("");
  let specFiles = $state<string[] | null>(null);
  let specLoaded = $state("");
  let specMd = $state("");
  let code = $state<{ path: string; text: string } | null>(null);
  let report = $state<any>(null);
  let rows = $state<Row[]>([]);
  let overlap = $state<any[] | null>(null);
  let uncovered = $state<any>(null);
  let trace = $state<{ text: string; data: any } | null>(null);
  let root = $state("");
  let error = $state("");

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

  async function loadSetup() {
    const [r, d] = await Promise.all([call("spec_install_status"), call("spec_doctor")]);
    setup = { ...r.data, text: r.text };
    checks = d.data.checks;
    indexAction = d.data.index_action;
  }

  // the only thing here that indexes: runs when the user clicks, never on its own
  async function runIndex(action: "init" | "sync") {
    if (!confirm(action === "init" ? "Run `codegraph init` in this repo? It creates .codegraph/ and can take a while." : "Run `codegraph sync` to refresh the index?")) return;
    busy = true;
    indexLog = `codegraph ${action} running…`;
    try {
      indexLog = (await call("spec_codegraph", { action })).text;
      await Promise.all([loadSetup(), load()]);
    } catch (e: any) {
      indexLog = String(e.message ?? e);
    } finally {
      busy = false;
    }
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

  // each view loads what its URL names
  $effect(() => {
    const id = route.id;
    if (!id) { trace = null; return; }
    if (untrack(() => trace?.data?.id) === id) return;
    call("spec_trace", { id }).then((r) => (trace = r)).catch((e) => { trace = null; error = e.message; });
  });

  $effect(() => {
    if (route.tab !== "specs") return;
    if (specFiles === null) { call("spec_file").then((r) => (specFiles = r.data.files)).catch((e) => (error = e.message)); return; }
    const f = route.file ?? specFiles[0];
    if (f && f !== untrack(() => specLoaded)) call("spec_file", { name: f }).then((r) => { specLoaded = f; specMd = r.data.markdown; }).catch((e) => (error = e.message));
  });

  $effect(() => {
    const path = route.tab === "code" ? route.path : undefined;
    if (path && untrack(() => code?.path) !== path) call("spec_source", { path }).then((r) => (code = r.data)).catch((e) => { code = null; error = e.message; });
  });

  $effect(() => {
    if (route.tab === "overlap" && overlap === null) call("spec_overlap").then((r) => (overlap = r.data.rows)).catch((e) => (error = e.message));
    if (route.tab === "uncovered" && uncovered === null) call("spec_uncovered").then((r) => (uncovered = r.data)).catch((e) => (error = e.message));
  });

  // scroll to what the URL points at once it is rendered
  $effect(() => {
    void [specMd, code, rows.length, route.id, route.line];
    queueMicrotask(() => {
      const el = route.tab === "code" ? document.getElementById(`L${route.line}`) : route.tab === "specs" && route.id ? document.querySelector(`article a[data-id="${route.id}"]`) : route.tab === "requirements" ? document.querySelector("tr.sel") : null;
      el?.scrollIntoView({ block: "center" });
    });
  });

  load();
  loadSetup().catch(() => {});

  // ---- views ----
  const states = ["all", "tested", "src-only", "none", "manual", "later"];
  const stateFilter = $derived(route.state ?? "all");
  const query = $derived(route.q ?? "");
  const shown = $derived(rows.filter((r) => (stateFilter === "all" || r.state === stateFilter) && (!query || `${r.id} ${r.text} ${r.file}`.toLowerCase().includes(query.toLowerCase()))));
  const counts = $derived(rows.reduce<Record<string, number>>((m, r) => ((m[r.state] = (m[r.state] ?? 0) + 1), m), { all: rows.length }));
  const invalidN = $derived(report ? Object.values(report.invalid_tests).reduce((n: number, v: any) => n + v.length, 0) : 0);
  const known = $derived(new Set(rows.map((r) => r.id)));
  const plain = (t: string) => t.replace(/[`*~]/g, "");
  const req = (id: string): Route => ({ tab: "requirements", id });
  const withId = (tab: Route["tab"], id: string): Route => ({ tab, id });
  // typing in the filter replaces the URL instead of stacking a history entry per keystroke
  const filter = (patch: Partial<Route>) => navigate({ ...route, ...patch }, true);
  const segs = (line: string) => line.split(/(\b[A-Z]{2,6}-\d{1,3}\b)/).map((t, i) => ({ t, id: i % 2 === 1 && known.has(t) }));
</script>

{#snippet idlink(id: string, to: Route)}
  <a class="id" href={u(to)}>{id}</a>
{/snippet}

{#snippet tracePanel()}
  {#if trace}
    {@const sp = trace.data?.spec}
    <aside>
      <div class="ahead">
        <b class="mono nowrap">{trace.data.id}</b>
        {#if sp}<span class="pill">{sp.level}</span>{/if}
        <a class="x" href={u({ ...route, id: undefined })} aria-label="close trace">✕</a>
      </div>
      {#if sp}
        <p class="muted mono nowrap"><a href={u({ tab: "specs", file: sp.file, id: trace.data.id })}>{sp.file}:{sp.line}</a></p>
        <Markdown source={sp.text} />
      {:else}<p class="muted">Not in any spec: cited only in tests or code.</p>{/if}
      {#if trace.data.tests?.length}
        <h4>Tests ({trace.data.tests.length})</h4>
        <ul class="refs">{#each trace.data.tests as t}<li><a class="mono nowrap" href={u({ tab: "code", path: t.file, line: t.line })}>{t.file}:{t.line}</a> {t.text}</li>{/each}</ul>
      {/if}
      {#if trace.data.source_cites?.length}
        <h4>Code citations ({trace.data.source_cites.length})</h4>
        <ul class="refs">{#each trace.data.source_cites as c}<li><a class="mono nowrap" href={u({ tab: "code", path: c.file, line: c.line })}>{c.file}:{c.line}</a> <code>{c.decl}</code></li>{/each}</ul>
      {/if}
      {#if trace.data.related?.length}
        <h4>Related code ({trace.data.related_total})</h4>
        <ul class="refs">{#each trace.data.related as r}<li><a class="mono nowrap" href={u({ tab: "code", path: r.file })}>{r.file}</a> <span class="muted">{r.types.join(", ")}{r.cites ? " · cites it" : ""}</span></li>{/each}</ul>
      {/if}
      <details><summary class="muted">plain text</summary><pre>{trace.text}</pre></details>
      <p class="muted note">Evidence, not truth.</p>
    </aside>
  {/if}
{/snippet}

<header>
  <h1><a href="/report">spec11</a></h1>
  <a class="root mono muted" href="/report">{root}</a>
  <nav aria-label="views">
    {#each TABS as t}
      <a class="tab" class:on={route.tab === t} aria-current={route.tab === t ? "page" : undefined} href={u({ tab: t })}>
        {t}{#if t === "report" && report?.total_problems}<span class="badge bad" title="{report.total_problems} problems">{report.total_problems}</span>{/if}{#if t === "setup" && setup?.suggestions.length}<span class="badge warn" title="not fully set up">!</span>{/if}
      </a>
    {/each}
  </nav>
  <button onclick={load} title="Rescan the repo" aria-label="rescan">↻</button>
  <div class="themes" role="group" aria-label="theme">
    {#each themes as t}<button class:on={theme === t.v} aria-pressed={theme === t.v} title={t.tip} aria-label={t.tip} onclick={() => (theme = t.v)}>{t.icon}</button>{/each}
  </div>
</header>

{#if error}<p class="error">{error}</p>{/if}
{#if setup && setup.suggestions.length && route.tab !== "setup"}
  <p class="banner">
    <span>spec11 is not fully set up for this repo: {setup.suggestions[0]}</span>
    <span class="grow"></span>
    <button disabled={busy} class="primary" onclick={() => runSetup("spec_install")}>Install here</button>
    <a class="tab" href="/setup">Details</a>
  </p>
{/if}

<main>
  {#if !report}
    <p class="muted">Scanning…</p>
  {:else if route.tab === "report"}
    <section class="hero card">
      <div class="pct"><b>{report.tested_pct}%</b><span>of due requirements have a tagged test <small class="muted">({report.tested}/{report.due})</small></span></div>
      <div class="meter" role="img" aria-label="{report.tested_pct}% tested"><i style="width:{report.tested_pct}%"></i></div>
      <p class="facts muted">
        <a href={u({ tab: "requirements" })}>{report.requirements} requirements</a> in {report.spec_files} specs
        {#if report.manual} · <a href={u({ tab: "requirements", state: "manual" })}>{report.manual} manual</a>{/if}
        {#if report.later} · <a href={u({ tab: "requirements", state: "later" })}>{report.later} later</a>{/if}
        · <b class={report.total_problems ? "bad" : "ok"}>{report.total_problems ? `${report.total_problems} problems` : "no problems found"}</b>
      </p>
    </section>
    {#if !report.index}<p class="note-line muted">No CodeGraph index: test-to-code links and uncovered code are not checked.</p>{/if}

    {#snippet ids(title: string, list: string[], sev: string)}
      {#if list.length}
        <details class={sev} open>
          <summary><span>{title}</span> <b class="count">{list.length}</b></summary>
          <p class="ids">{#each list as id}{@render idlink(id, req(id))}{/each}</p>
        </details>
      {/if}
    {/snippet}
    {@render ids("Unimplemented: due, no test and no code", report.unimplemented, "bad")}
    {@render ids("Untested: due, cited in code but no test", report.untested, "warn")}
    {@render ids("Marked later but already has tests or code", report.built, "warn")}

    {#if report.unproven.length}
      <details class="warn" open>
        <summary><span>Check-marked done without proof</span> <b class="count">{report.unproven.length}</b></summary>
        <ul>{#each report.unproven as x}<li>{@render idlink(x.id, req(x.id))} <code>{x.verdict}</code> <a href={u({ tab: "specs", file: x.file, id: x.id })}>{x.file}:{x.line}</a> <span class="muted">{plain(x.text).slice(0, 80)}</span></li>{/each}</ul>
      </details>
    {/if}
    {#if invalidN}
      <details class="bad" open>
        <summary><span>Invalid tests</span> <b class="count">{invalidN}</b></summary>
        {#each Object.entries(report.invalid_tests) as [kind, list]}
          {#if (list as any[]).length}<p><code>{kind}</code> {#each list as any[] as p}{@render idlink(p.id, req(p.id))} <a class="muted mono" href={u({ tab: "code", path: p.file, line: p.line })}>{p.file}:{p.line}</a> {/each}</p>{/if}
        {/each}
      </details>
    {/if}
    {#if report.spec_issues.length}
      <details class="bad" open>
        <summary><span>Spec issues</span> <b class="count">{report.spec_issues.length}</b></summary>
        <ul>{#each report.spec_issues as x}<li class="mono">{x}</li>{/each}</ul>
      </details>
    {/if}
    {#if report.dangling_tests.length + report.dangling_sources.length}
      <details class="warn" open>
        <summary><span>Dangling citations</span> <b class="count">{report.dangling_tests.length + report.dangling_sources.length}</b></summary>
        <p class="mono">{#if report.dangling_tests.length}tests: {report.dangling_tests.join(", ")}<br />{/if}{#if report.dangling_sources.length}sources: {report.dangling_sources.join(", ")}{/if}</p>
      </details>
    {/if}
  {:else if route.tab === "requirements"}
    <div class="bar">
      <div class="chips" role="group" aria-label="state">
        {#each states as st}{#if st === "all" || counts[st]}<button class="chip {st}" class:on={stateFilter === st} aria-pressed={stateFilter === st} onclick={() => filter({ state: st })}>{st} <b>{counts[st] ?? 0}</b></button>{/if}{/each}
      </div>
      <input name="filter" aria-label="filter" placeholder="Filter ID, text, file   ( / )" value={query} oninput={(e) => filter({ q: e.currentTarget.value })} />
    </div>
    <div class="split">
      <table>
        <tbody>
          {#each shown.slice(0, 500) as r (r.id)}
            <tr onclick={() => navigate({ ...route, id: r.id })} class:sel={route.id === r.id} class="st-{r.state}">
              <td class="mono nowrap"><a href={u({ ...route, id: r.id })}>{r.id}</a></td>
              <td><span class="pill {r.state}">{r.state}</span>{#if r.done}<span class="pill done" title="check-marked: {r.verdict}">✓ {r.verdict}</span>{/if}</td>
              <td class="txt" title={plain(r.text)}>{plain(r.text)}</td>
              <td class="mono muted nowrap"><a href={u({ tab: "specs", file: r.file, id: r.id })}>{r.file}:{r.line}</a></td>
            </tr>
          {/each}
        </tbody>
      </table>
      {@render tracePanel()}
    </div>
    {#if !shown.length}<p class="muted empty">No requirement matches. <a href={u({ tab: "requirements" })}>Clear filters</a></p>{/if}
    {#if shown.length > 500}<p class="muted">First 500 shown; narrow the filter.</p>{/if}
  {:else if route.tab === "code"}
    {#if !route.path}<p class="muted">No file selected: open one from a requirement's tests or citations.</p>
    {:else if !code || code.path !== route.path}<p class="muted">Loading {route.path}…</p>
    {:else}
      <p class="mono nowrap crumb"><a href="/requirements">requirements</a> / {code.path}{route.line ? `:${route.line}` : ""}</p>
      <table class="code">
        <tbody>
          {#each code.text.split("\n") as l, i}
            <tr id={`L${i + 1}`} class:hl={route.line === i + 1}>
              <td class="ln"><a href={u({ tab: "code", path: code.path, line: i + 1 })}>{i + 1}</a></td>
              <td class="src">{#each segs(l) as p}{#if p.id}<a class="id" href={u(req(p.t))}>{p.t}</a>{:else}{p.t}{/if}{/each}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  {:else if route.tab === "setup"}
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
              <td>
                {k.detail}{#if k.hint}<div class="muted">→ {k.hint}</div>{/if}
                {#if k.name === "codegraph index" && indexAction}
                  <div><button class="primary" disabled={busy} onclick={() => runIndex(indexAction!)}>{indexAction === "init" ? "Create the index (codegraph init)" : "Refresh the index (codegraph sync)"}</button></div>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if indexLog}<pre class="log">{indexLog}</pre>{/if}
    {/if}
  {:else if route.tab === "specs"}
    {#if specFiles === null}<p class="muted">Loading…</p>
    {:else}
      <div class="specs">
        <ul class="files">{#each specFiles as f}<li><a class="fileLink" class:on={f === (route.file ?? specFiles[0])} href={u({ tab: "specs", file: f })}>{f.replace(/\.md$/, "")}</a></li>{/each}</ul>
        <article><Markdown source={specMd} /></article>
      </div>
    {/if}
  {:else if route.tab === "overlap"}
    {#if overlap === null}<p class="muted">Loading…</p>
    {:else if overlap.length === 0}<p class="muted">No declaration is covered by two or more requirements (or no CodeGraph index).</p>
    {:else}
      {#each overlap as r}
        <div class="card block">
          <div class="mono nowrap"><a href={u({ tab: "code", path: r.file, line: r.line })}>{r.file}:{r.line}</a> <b>{r.name}</b></div>
          {#each r.pairs as [a, b, linked]}
            <div>{@render idlink(a, withId("overlap", a))} {linked ? "~ linked" : "? UNLINKED"} {@render idlink(b, withId("overlap", b))}</div>
          {/each}
        </div>
      {/each}
      {@render tracePanel()}
    {/if}
  {:else if route.tab === "uncovered"}
    {#if uncovered === null}<p class="muted">Loading…</p>
    {:else if !uncovered.files}<p class="muted">Not available (no CodeGraph index).</p>
    {:else}
      <p>{uncovered.units} units / {uncovered.lines} lines in {uncovered.files.length} files have no requirement. Candidates, not proof.</p>
      <table>
        <tbody>
          {#each uncovered.files as f}
            <tr>
              <td class="mono"><a href={u({ tab: "code", path: f.file })}>{f.file}</a></td>
              <td>{f.lines} lines</td>
              <td class="mono muted">{#each f.units as un}<a href={u({ tab: "code", path: f.file, line: un.lo })}>{un.name}:{un.lo}-{un.hi}</a>{" "}{/each}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  {/if}
</main>

<footer class="muted">Evidence, not truth: tags and citations do not prove a requirement is implemented or its test passes.</footer>

<style>
  /* layout */
  header { position: sticky; top: 0; z-index: 2; display: flex; align-items: center; gap: 10px; padding: 10px 20px; background: color-mix(in srgb, var(--bg) 92%, transparent); backdrop-filter: blur(8px); border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  h1 { margin: 0; font-size: 18px; letter-spacing: -0.01em; }
  nav { margin-left: auto; display: flex; gap: 6px; flex-wrap: wrap; }
  main { padding: 20px; max-width: 1240px; margin: 0 auto; }
  footer { padding: 24px 16px; text-align: center; font-size: 12px; }
  .muted { color: var(--muted); }
  .nowrap { white-space: nowrap; }
  .grow { flex: 1; }
  .error { margin: 12px 20px; padding: 10px 14px; border: 1px solid var(--bad); background: color-mix(in srgb, var(--bad) 10%, var(--card)); color: var(--bad); border-radius: var(--radius); }
  .banner { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin: 0; padding: 8px 20px; background: color-mix(in srgb, var(--warn) 13%, var(--bg)); border-bottom: 1px solid color-mix(in srgb, var(--warn) 35%, var(--line)); font-size: 13px; }
  button:disabled { opacity: 0.5; cursor: default; }

  /* header controls */
  a.tab { color: var(--fg); background: var(--raised); text-decoration: none; border-radius: var(--radius); padding: 5px 12px; text-transform: capitalize; }
  a.tab:hover { background: color-mix(in srgb, var(--accent) 16%, var(--raised)); color: var(--accent); }
  a.tab.on { background: var(--accent); color: var(--card); font-weight: 600; }
  h1 a, a.root { color: inherit; text-decoration: none; }
  a.root { color: var(--muted); max-width: 32ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; direction: rtl; text-align: left; }
  .badge { display: inline-block; min-width: 18px; margin-left: 6px; padding: 0 5px; border-radius: 2px; font-size: 11px; font-weight: 700; line-height: 18px; text-align: center; color: var(--card); }
  .badge.bad { background: var(--bad); } .badge.warn { background: var(--warn); }
  a.tab.on .badge { outline: 1px solid var(--card); }
  .themes { display: flex; gap: 2px; }
  .themes button { padding: 5px 10px; }
  

  /* report */
  .card { background: var(--card); border: 1px solid var(--line); border-radius: var(--radius); padding: 14px 16px; box-shadow: var(--shadow); }
  .hero { margin-bottom: 14px; }
  .pct { display: flex; align-items: baseline; gap: 12px; flex-wrap: wrap; }
  .pct b { font-size: 34px; line-height: 1.1; letter-spacing: -0.02em; }
  .meter { height: 8px; margin: 10px 0 8px; border-radius: 4px; background: var(--raised); overflow: hidden; }
  .meter i { display: block; height: 100%; background: var(--ok); border-radius: 4px; }
  .facts { margin: 0; font-size: 13px; }
  b.bad { color: var(--bad); } b.ok { color: var(--ok); }
  .note-line { font-size: 13px; margin: 0 0 12px; }
  details { background: var(--card); border: 1px solid var(--line); border-left: 4px solid var(--line); border-radius: var(--radius); padding: 10px 14px; margin-bottom: 10px; }
  details.bad { border-left-color: var(--bad); background: color-mix(in srgb, var(--bad) 5%, var(--card)); }
  details.warn { border-left-color: var(--warn); background: color-mix(in srgb, var(--warn) 5%, var(--card)); }
  summary { cursor: pointer; display: flex; align-items: center; gap: 8px; font-weight: 600; }
  summary span { flex: 1; }
  details > :not(summary) { margin-top: 10px; }
  .count { font-size: 12px; padding: 0 7px; border-radius: 2px; color: var(--card); }
  details.bad .count { background: var(--bad); } details.warn .count { background: var(--warn); }
  details ul { margin: 0; padding-left: 18px; } details li { margin: 4px 0; }
  .ids { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 0; }

  /* ids, links, pills */
  a.id { font-family: var(--mono); font-size: 12.5px; padding: 1px 7px; white-space: nowrap; text-decoration: none; border: 1px solid var(--line); border-radius: 6px; background: var(--card); }
  a.id:hover { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 10%, var(--card)); }
  td a, .refs a, li a { text-decoration: none; }
  td a:hover, .refs a:hover, li a:hover { text-decoration: underline; }
  .pill { font-size: 11.5px; padding: 1px 8px; border-radius: 2px; border: 0; margin-right: 4px; white-space: nowrap; background: color-mix(in srgb, currentColor 10%, transparent); }
  .pill.tested { color: var(--ok); } .pill.none { color: var(--bad); } .pill.src-only { color: var(--warn); } .pill.done { color: var(--accent); }
  .pill.manual, .pill.later { color: var(--muted); }

  /* requirements */
  .bar { display: flex; gap: 10px; align-items: center; margin-bottom: 14px; flex-wrap: wrap; }
  .bar input { flex: 1; min-width: 200px; }
  .chips { display: flex; gap: 6px; flex-wrap: wrap; }
  .chip { font-size: 13px; padding: 3px 10px; }
  .chip b { font-weight: 600; opacity: 0.75; margin-left: 2px; }
  
  .chip.none:not(.on) b, .chip.none:not(.on) { color: var(--bad); }
  .chip.src-only:not(.on) { color: var(--warn); }
  .split { display: grid; grid-template-columns: minmax(0, 1fr); gap: 14px; align-items: start; }
  @media (min-width: 1000px) { .split:has(aside) { grid-template-columns: minmax(0, 3fr) minmax(0, 2fr); } }
  table { width: 100%; border-collapse: collapse; }
  td { padding: 7px 10px; border-bottom: 1px solid var(--line); vertical-align: top; }
  tr[onclick] { cursor: pointer; }
  tr[onclick] td:first-child { border-left: 3px solid transparent; }
  tr.st-none td:first-child { border-left-color: var(--bad); }
  tr.st-src-only td:first-child { border-left-color: var(--warn); }
  tr.st-tested td:first-child { border-left-color: var(--ok); }
  tr.sel, tr[onclick]:hover { background: color-mix(in srgb, var(--accent) 9%, transparent); }
  .txt { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .empty { padding: 24px 0; text-align: center; }
  aside { position: sticky; top: 64px; background: var(--card); border: 1px solid var(--line); border-radius: var(--radius); padding: 10px 16px 14px; overflow: auto; max-height: calc(100vh - 80px); box-shadow: var(--shadow); }
  @media (max-width: 999px) { aside { order: -1; position: static; max-height: none; } }
  .ahead { display: flex; align-items: center; gap: 8px; }
  .ahead .x { margin-left: auto; text-decoration: none; padding: 0 8px; color: var(--muted); }
  aside h4 { margin: 14px 0 4px; font-size: 13px; }
  .refs { margin: 0; padding-left: 16px; font-size: 13px; }
  .note { font-size: 12px; margin: 10px 0 0; }
  pre { margin: 0; white-space: pre-wrap; word-break: break-word; }

  /* specs, code, setup */
  .specs { display: grid; grid-template-columns: 220px minmax(0, 1fr); gap: 16px; align-items: start; }
  .files { list-style: none; margin: 0; padding: 0; position: sticky; top: 64px; max-height: 80vh; overflow: auto; display: flex; flex-direction: column; gap: 2px; }
  a.fileLink { display: block; color: inherit; text-decoration: none; border-radius: var(--radius); padding: 5px 10px; }
  a.fileLink:hover { background: var(--raised); }
  a.fileLink.on { background: var(--raised); color: var(--accent); font-weight: 600; }
  article { background: var(--card); border: 1px solid var(--line); border-radius: var(--radius); padding: 4px 20px 14px; box-shadow: var(--shadow); }
  @media (max-width: 800px) { .specs { grid-template-columns: 1fr; } .files { position: static; max-height: 30vh; } }
  .crumb { margin: 0 0 8px; }
  table.code { font-family: var(--mono); font-size: 12.5px; background: var(--card); border: 1px solid var(--line); border-radius: var(--radius); }
  table.code td { border: none; padding: 0 10px; white-space: pre; }
  td.ln { text-align: right; width: 1%; user-select: none; }
  td.ln a { color: var(--muted); }
  tr.hl { background: color-mix(in srgb, var(--warn) 22%, transparent); }
  td.src { overflow-x: auto; }
  .block { margin-bottom: 10px; }
  .setup h3 { margin: 0 0 6px; font-size: 15px; }
  .setup p { margin: 6px 0; }
  .log { margin-top: 12px; padding: 10px 14px; background: var(--raised); border-radius: var(--radius); font-family: var(--mono); font-size: 12.5px; }
</style>
