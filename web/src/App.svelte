<script lang="ts">
  import { untrack } from "svelte";
  import { call, embedded, onToolInput, read, watch } from "./transport";
  import { TABS, format, parse, type Route } from "./router";
  import Markdown from "./Markdown.svelte";
  import Graph from "./Graph.svelte";
  import Split from "./Split.svelte";
  import SplitToggle from "./SplitToggle.svelte";
  import DataTable from "./DataTable.svelte";
  import Seg from "./Seg.svelte";
  import Pill from "./Pill.svelte";
  import IdLink from "./IdLink.svelte";
  import Section from "./Section.svelte";
  import PaneHead from "./PaneHead.svelte";
  import { stateTone, edgeTone, levelTone, changeTone, type Tone } from "./tone";
  import { preview } from "./layout.svelte";
  import { reviewed, loadReviewed, setReviewed } from "./reviewed.svelte";
  import DiffView from "./DiffView.svelte";
  import ViewSwitch from "./ViewSwitch.svelte";
  import { sorted, toggleSort, type Sort } from "./sort";
  import { inline } from "./markdown";
  import { highlightLines } from "./highlight";

  type Ref = { file: string; line: number; text: string };
  type Row = { id: string; state: string; file: string; line: number; level: string; text: string; mark: string; done: boolean; verdict: string; built: boolean; issues: string[]; tests: Ref[]; cites: Ref[]; graph: string[] };

  // ---- routing: the URL is the state ----
  const here = (): Route => {
    try { return parse(new URL(location.href)); } catch { return { tab: "dashboard" }; }
  };
  let route = $state<Route>(here());
  const u = format;

  function navigate(r: Route, replace = false) {
    route = r;
    if (embedded) return; // a host iframe has no URL of its own
    try { (replace ? window.history.replaceState : window.history.pushState).call(window.history, null, "", format(r)); } catch { /* sandboxed */ }
  }

  // MCP Apps host: the `dashboard` tool's `view` ("requirements/DOC-25?state=none") picks the page
  onToolInput((a) => typeof a.view === "string" && navigate(parse(new URL(a.view.replace(/^\/?/, "/"), location.href))));

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
    document.title = ["spec17", route.tab, route.id ?? route.file ?? route.path].filter(Boolean).join(" · ");
  });

  // setup banner: hidden per browser until its message changes
  const stored = (k: string) => { try { return localStorage.getItem(k) ?? ""; } catch { return ""; } };
  let dismissed = $state(stored("spec17.banner"));
  const dismiss = (m: string) => { dismissed = m; try { localStorage.setItem("spec17.banner", m); } catch { /* storage blocked */ } };
  // nav emoji; below 1350px only these show (the label is hidden, the title keeps the name)
  const EMOJI: Record<string, string> = { dashboard: "📊", changes: "📝", requirements: "📋", modules: "📦", tests: "🧪", specs: "📄", overlap: "🔀", uncovered: "🚫" };
  const plural = (n: number, w: string) => `${n} ${w}${n === 1 ? "" : "s"}`;

  // ---- theme: auto (OS) | light | dark, remembered per browser ----
  type Theme = "auto" | "light" | "dark";
  // one button cycles system → light → dark; it shows the current state
  const themes: { v: Theme; icon: string; name: string }[] = [{ v: "auto", icon: "◐", name: "system" }, { v: "light", icon: "☀", name: "light" }, { v: "dark", icon: "☾", name: "dark" }];
  const curTheme = $derived(themes.find((t) => t.v === theme)!);
  const nextTheme = $derived(themes[(themes.indexOf(curTheme) + 1) % themes.length]);
  const saved = (): Theme => { try { const t = localStorage.getItem("spec17.theme"); return t === "light" || t === "dark" ? t : "auto"; } catch { return "auto"; } };
  let theme = $state<Theme>(saved());
  $effect(() => {
    const el = document.documentElement;
    if (theme === "auto") delete el.dataset.theme; else el.dataset.theme = theme;
    try { theme === "auto" ? localStorage.removeItem("spec17.theme") : localStorage.setItem("spec17.theme", theme); } catch { /* storage blocked */ }
  });

  // keyboard: `/` focuses the filter (opening the requirements list first), filter shortcuts, Esc closes the trace panel (↑/↓ on the lists: DataTable)
  $effect(() => {
    const onkey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (t.closest("input, select, textarea, .gutter, .seg") || e.metaKey || e.ctrlKey || e.altKey) { if (e.key === "Escape") (t as HTMLElement).blur(); return; }
      if (e.key === "/") {
        e.preventDefault();
        if (route.tab !== "requirements") navigate({ tab: "requirements" });
        queueMicrotask(() => document.querySelector<HTMLInputElement>("input[name=filter]")?.focus());
      } else if (route.tab === "requirements" && /^[0-7sl]$/i.test(e.key)) {
        // filter shortcuts: 1-7 pick a state, S/L step through states/levels (⇧ goes back), 0 clears every filter
        const k = e.key.toLowerCase(), d = e.shiftKey ? -1 : 1;
        const step = (keys: string[], cur: string) => keys[(keys.indexOf(cur) + d + keys.length) % keys.length];
        if (k === "0") filter({ state: undefined, level: undefined, q: undefined, changed: undefined, reviewed: undefined });
        else if (k === "s") filter({ state: step(states, stateFilter) });
        else if (k === "l") { const n = step(levelKeys, levelFilter || "all"); filter({ level: n === "all" ? undefined : n }); }
        else if (+k <= states.length) filter({ state: states[+k - 1] });
      } else if (e.key === "Escape" && route.id && (route.tab === "requirements" || route.tab === "overlap")) navigate({ ...route, id: undefined });
      else if (e.key === "Escape" && route.path && route.tab === "tests") navigate({ ...route, path: undefined, line: undefined });
    };
    document.addEventListener("keydown", onkey);
    return () => document.removeEventListener("keydown", onkey);
  });

  // ---- data ----
  let setup = $state<{ status: any; suggestions: string[]; text: string; version?: string } | null>(null);
  let setupError = $state("");
  let busy = $state(false);
  let checks = $state<{ name: string; level: string; detail: string; hint: string | null }[]>([]);
  let indexAction = $state<"init" | "sync" | null>(null);
  let indexLog = $state("");
  let specFiles = $state<string[] | null>(null);
  let specLoaded = $state("");
  let specMd = $state("");
  let code = $state<{ path: string; text: string } | null>(null);
  let report = $state<any>(null);
  // the working tree against HEAD: undefined until loaded, null when it cannot be read (not a git work tree)
  let changes = $state<any>(undefined);
  // (WEB-35, WEB-36) {requirement ID: its newest commit (sha, short, author, date, subject)}; empty without git
  let commits = $state<Record<string, any>>({});
  let custom = $state<any>(undefined);
  // commits not on the base branch, for the Changes page
  let history = $state<any>(null);
  let openDiffs = $state<string[]>([]);
  const toggleDiff = (p: string) => (openDiffs = openDiffs.includes(p) ? openDiffs.filter((x) => x !== p) : [...openDiffs, p]);
  let modules = $state<any>(null);
  let tests = $state<any>(null);
  let rows = $state<Row[]>([]);
  let overlap = $state<any[] | null>(null);
  let uncovered = $state<any>(null);
  let trace = $state<{ text: string; data: any } | null>(null);
  let root = $state("");
  let error = $state("");

  const guard = <T,>(p: Promise<T>) => p.catch((e) => { error = String(e.message ?? e); throw e; });

  // server connection: null until known; the push stream sets it, and so does every successful load or poll (the only signal when embedded)
  let up = $state<boolean | null>(null);
  // bumped by every load: the views below refetch (keeping what they show until the new data lands)
  let gen = $state(0);
  let loadId = 0;
  async function load() {
    error = "";
    const me = ++loadId;
    const [r, s] = await guard(Promise.all([call("report", { limit: 1000 }), call("status", { all: true, limit: 5000 })]));
    if (me !== loadId) return; // a newer load started meanwhile: its answer wins
    report = r.data;
    rows = s.data.rows;
    up = true;
    // the modules views are optional: a failed resource read leaves them empty instead of failing the load
    read("spec17://modules").then((m) => (modules = m)).catch(() => (modules = null));
    call("changes").then((c) => (changes = c.data)).catch(() => (changes = null));
    call("commits").then((c) => (commits = c.data.commits)).catch(() => (commits = {}));
    gen++;
    if (!root) fetch("/api/info").then((x) => x.json()).then((i) => (root = i.root ?? "")).catch(() => {}).finally(() => loadReviewed(root));
  }

  // (WEB-10, WEB-11) hide a module (after a confirm) or show it again via .spec17ignore, then rescan
  async function setHidden(path: string, hidden: boolean) {
    if (hidden && !confirm(`Add /${path}/ to .spec17ignore? spec17 will leave the module out of every scan.`)) return;
    try {
      await guard(call("module", { path, hidden }));
      await load();
    } catch { /* guard shows it */ }
  }

  // (WEB-27) Setup needs only these two calls, never the scan; a failure is shown there with a retry, not left at "Checking…"
  async function loadSetup() {
    try {
      const [r, d] = await Promise.all([call("install_status"), call("doctor")]);
      setup = { ...r.data, text: r.text };
      checks = d.data.checks;
      indexAction = d.data.index_action;
      setupError = "";
    } catch (e: any) {
      setupError = String(e.message ?? e);
    }
  }

  // (WEB-30) the rescan button: sync the index (`refresh` syncs even beside a running codegraph), then reload; a failed sync is shown, the reload still happens
  async function rescan() {
    busy = true;
    let sync = "";
    try { await call("codegraph", { action: "refresh" }); } catch (e: any) { sync = String(e.message ?? e); }
    await Promise.allSettled([load(), loadSetup()]);
    if (sync) error = sync;
    busy = false;
  }

  // the only thing here that indexes: runs when the user clicks, never on its own
  async function runIndex(action: "init" | "sync") {
    if (!confirm(action === "init" ? "Run `codegraph init` in this repo? It creates .codegraph/ and can take a while." : "Run `codegraph sync` to refresh the index?")) return;
    busy = true;
    indexLog = `codegraph ${action} running…`;
    try {
      indexLog = (await call("codegraph", { action })).text;
      await Promise.all([loadSetup(), load()]);
    } catch (e: any) {
      indexLog = String(e.message ?? e);
    } finally {
      busy = false;
    }
  }

  // `server: "codegraph"` (WEB-29) only adds or removes the CodeGraph entry of .mcp.json
  async function runSetup(tool: "install" | "uninstall", global = false, options: { server?: "codegraph"; only?: "skill" | "mcp"; transport?: "stdio" | "http" } = {}) {
    busy = true;
    try {
      await guard(call(tool, { global, ...options }));
      await loadSetup(); // the status and the checks name what is still missing
    } finally {
      busy = false;
    }
  }

  function launchCommand(launch?: { command: string; args: string[] } | null) {
    if (!launch) return undefined;
    return [launch.command, ...launch.args].map((arg) => /^[\w./:-]+$/.test(arg) ? arg : "'" + arg.replaceAll("'", "'\"'\"'") + "'").join(" ");
  }

  // each view loads what its URL names
  $effect(() => {
    const id = route.id;
    void rows; // a refresh replaces the rows and must reload the open trace
    if (!id) { trace = null; return; }
    // the previous trace stays (dimmed) until the new one lands, so the preview pane never collapses and reopens
    call("trace", { id }).then((r) => { if (route.id === id) { trace = r; document.querySelector(".split .preview")?.scrollTo(0, 0); } }).catch((e) => { trace = null; error = e.message; });
  });

  $effect(() => {
    if (route.tab !== "specs") return;
    if (specFiles === null) { call("file").then((r) => (specFiles = r.data.files)).catch((e) => (error = e.message)); return; }
    const f = route.file ?? specFiles[0];
    if (f && f !== untrack(() => specLoaded)) call("file", { name: f }).then((r) => { specLoaded = f; specMd = r.data.markdown; document.querySelector(".split .preview")?.scrollTo(0, 0); }).catch((e) => (error = e.message));
  });

  $effect(() => {
    const path = route.tab === "code" || route.tab === "tests" ? route.path : undefined;
    if (path && untrack(() => code?.path) !== path) call("source", { path }).then((r) => (code = r.data)).catch((e) => { code = null; error = e.message; });
  });

  // fetched once per load generation, when its tab is open
  const fetched: Record<string, number> = {};
  const fetchOnce = (key: string, get: () => Promise<void>) => {
    if (fetched[key] === gen) return;
    fetched[key] = gen;
    get().catch((e) => { fetched[key] = -1; error = e.message; });
  };
  $effect(() => {
    if (route.tab === "overlap") fetchOnce("overlap", () => call("overlap").then((r) => void (overlap = r.data.rows)));
    if (route.tab === "tests" || route.tab === "requirements" || route.tab === "changes" || (route.tab === "modules" && route.module)) fetchOnce("tests", () => read("spec17://tests").then((r) => void (tests = r)));
    if (route.tab === "uncovered") fetchOnce("uncovered", () => call("uncovered").then((r) => void (uncovered = r.data)));
  });

  // the Changes page asks for its own scope when it is not the default (staged + unstaged + untracked)
  const scope = $derived({
    ref: route.ref ?? "", commit: route.commit ?? "", staged: route.staged === "1", include_untracked: route.untracked !== "0",
    ignore_whitespace: route.ws === "1", show_ignored: route.ignored === "1", show_all: route.all === "1",
  });
  const defaultScope = $derived(!scope.ref && !scope.commit && !scope.staged && scope.include_untracked && !scope.ignore_whitespace && !scope.show_ignored && !scope.show_all);
  $effect(() => {
    if (route.tab !== "changes") return;
    void gen;
    call("history", { ignore_whitespace: scope.ignore_whitespace, show_ignored: scope.show_ignored, show_all: scope.show_all }).then((r) => (history = r.data)).catch(() => (history = null));
    if (defaultScope) return;
    call("changes", scope).then((r) => (custom = r.data)).catch(() => (custom = null));
  });

  // scroll to what the URL points at once it is rendered (the lists scroll their own selection: DataTable)
  $effect(() => {
    void [specMd, code, route.id, route.line];
    queueMicrotask(() => {
      const el = route.tab === "code" ? document.getElementById(`L${route.line}`) : route.tab === "specs" && route.id ? document.querySelector(`article a[data-id="${route.id}"]`) : null;
      el?.scrollIntoView({ block: "center" });
    });
  });

  // (WEB-27) the first run lands on Setup: nothing to scan and nothing installed, or a failed check; never inside a host that picks the view
  Promise.allSettled([load(), loadSetup()]).then(() => {
    const first = report && report.requirements + report.refs === 0 && setup?.suggestions.length;
    if (!embedded && route.tab === "dashboard" && (first || checks.some((k) => k.level === "fail"))) navigate({ tab: "setup" }, true);
  });
  // the server pushes `resources/updated` when the repo changes; one notification per subscribed resource arrives for one change
  let reloadTimer: ReturnType<typeof setTimeout> | undefined;
  // throttled: the first push reloads at once, a burst after it collapses into one trailing reload at most every 2 s
  let lastLoad = 0;
  $effect(() => watch(["spec17://report", "spec17://status", "spec17://changes"], () => {
    clearTimeout(reloadTimer);
    reloadTimer = setTimeout(() => { lastLoad = Date.now(); load().catch(() => {}); }, Math.max(0, lastLoad + 2000 - Date.now()));
  }, (s) => (up = s)));
  // fallback when no push arrives (embedded host, a dropped stream, a proxy that buffers): every 30 s, reload only if the report changed
  $effect(() => {
    const t = setInterval(async () => {
      if (document.hidden) return;
      try { if (JSON.stringify((await call("report", { limit: 1000 })).data) !== JSON.stringify(untrack(() => report))) await load(); up = true; } catch { up = false; /* the next tick retries */ }
    }, 30_000);
    return () => clearInterval(t);
  });

  // ---- views ----
  // the states that need work come first; the hints say what the scanner saw, not that anything passes
  const noReqs = $derived(report && report.requirements + report.refs === 0);
  const sum = (key: string) => (modules?.modules ?? []).reduce((n: number, m: any) => n + m[key], 0);
  const states = ["all", "none", "src-only", "tested", "manual", "later", "ref"];
  const STATE_HINT: Record<string, string> = {
    all: "Every requirement",
    none: "No test tag and no source citation",
    "src-only": "Cited in source, no test tag",
    tested: "A test carries its [ID] tag (run the suite to know it passes)",
    manual: "Checked by a person",
    later: "Marked later or deferred",
    ref: "Reference document, never due",
  };
  // (WEB-22) what the working tree changed: files by path, the kinds a requirement is touched by, the modules holding changed files
  const fileOf = $derived(new Map<string, any>((changes?.files ?? []).map((f: any) => [f.path, f])));
  // (WEB-37) directly changed: its bullet, a tagged test or a cited unit differs from the base; one only reached through a test is indirect
  const isDirect = (r: any) => r.notes.some((n: any) => n.action !== "reached by a test");
  const directReqs = $derived<any[]>((changes?.requirements ?? []).filter(isDirect));
  const changeOf = $derived(new Map<string, any>(directReqs.map((r: any) => [r.id, r])));
  const reqKinds = $derived(new Map<string, string[]>(directReqs.map((r: Row & { changed: string[] }) => [r.id, r.changed])));
  const moduleOfFile = (path: string) => (modules?.modules ?? []).map((m: any) => m.path).filter((m: string) => m !== "." && path.startsWith(`${m}/`)).sort((a: string, b: string) => b.length - a.length)[0] ?? ".";
  const changedMods = $derived(new Set<string>((changes?.files ?? []).map((f: any) => moduleOfFile(f.path))));
  const onlyChanged = $derived(route.changed === "1");
  // (WEB-35) reviewed = its commit is the one it was reviewed at; committed again since, it is unreviewed again
  const isReviewed = (id: string) => !!commits[id] && reviewed[id] === commits[id].sha;
  const onlyUnreviewed = $derived(route.reviewed === "0");
  const reviewTip = (id: string) => (isReviewed(id) ? `reviewed at ${commits[id].short}` : reviewed[id] ? `changed since reviewed (${reviewed[id].slice(0, 7)} → ${commits[id].short})` : "mark reviewed");
  // (WEB-36) who last changed the line of a test: asked per file when a preview shows it; keyed by reload (gen), so a reload asks again
  let blame = $state<Record<string, any>>({});
  const blameKey = (file: string, line: number) => `${gen}:${file}:${line}`;
  const wantBlame = (refs: { file: string; line: number }[]) => {
    const by = new Map<string, number[]>();
    for (const r of refs) if (!(blameKey(r.file, r.line) in blame)) by.set(r.file, [...(by.get(r.file) ?? []), r.line]);
    for (const [path, lines] of by) call("blame", { path, lines }).then((b) => { for (const [n, c] of Object.entries<any>(b.data.lines)) blame[blameKey(path, +n)] = c; }).catch(() => {});
  };
  $effect(() => { void gen; const t = trace?.data?.tests; if (t?.length) untrack(() => wantBlame(t)); });
  $effect(() => { void gen; if (route.tab === "tests" && route.path && route.line) untrack(() => wantBlame([{ file: route.path!, line: route.line! }])); });
  const changeMark = (path: string) => fileOf.get(path)?.change ?? "";
  // (WEB-25) the preview of a changed item shows the diff when the remembered view is "diff"
  const diffOn = (changed: boolean) => changed && preview.view === "diff";
  // the changed files behind a requirement: its spec file when the bullet was edited, and its tests and citing sources that changed
  const reqFiles = (t: any, kinds: string[] = []) => [...new Set<string>([...(kinds.includes("spec") && t.spec ? [t.spec.file] : []), ...(t.tests ?? []).map((x: any) => x.file), ...(t.source_cites ?? []).map((x: any) => x.file)].filter((f) => fileOf.has(f)))];
  const stateFilter = $derived(route.state ?? "all");
  const query = $derived(route.q ?? "");
  const levels = ["MUST", "SHOULD", "MAY"];
  const levelFilter = $derived(route.level ?? "");
  // SHALL and reference docs have no chip of their own: they group under "other"
  const levelOf = (r: Row) => (levels.includes(r.level) ? r.level : "other");
  // the rows that pass every filter except the one named: what a chip of that filter would show
  const facet = (skip?: "state" | "level") => rows.filter((r) => (skip === "state" || stateFilter === "all" || r.state === stateFilter) && (skip === "level" || !levelFilter || levelOf(r) === levelFilter) && (!onlyChanged || reqKinds.has(r.id)) && (!onlyUnreviewed || !isReviewed(r.id)) && (!query || `${r.id} ${r.text} ${r.file}`.toLowerCase().includes(query.toLowerCase())));
  const tally = (list: Row[], key: (r: Row) => string) => list.reduce<Record<string, number>>((m, r) => ((m[key(r)] = (m[key(r)] ?? 0) + 1), m), {});
  const shown = $derived(facet());
  const stateN = $derived.by(() => { const l = facet("state"); return { ...tally(l, (r) => r.state), all: l.length } as Record<string, number>; });
  const levelN = $derived.by(() => { const l = facet("level"); return { ...tally(l, levelOf), all: l.length } as Record<string, number>; });
  const levelKeys = $derived(["all", ...levels, ...(levelN.other || levelFilter === "other" ? ["other"] : [])]);
  const filtering = $derived(stateFilter !== "all" || !!levelFilter || !!query || onlyChanged || onlyUnreviewed);
  // a "changed only" switch with its count; shown once something changed (or while it is on, so it can be turned off)
  const changedSeg = (n: number) => [{ key: "all", label: "all" }, { key: "changed", label: "changed", count: n }];
  const showChanged = $derived(!!changes?.files.length || onlyChanged);
  const pickReviewed = (k: string) => filter({ reviewed: k === "unreviewed" ? "0" : undefined });
  const pickChanged = (k: string) => filter({ changed: k === "changed" ? "1" : undefined });
  const stateOpts = $derived(states.map((st, i) => ({ key: st, label: st, count: stateN[st] ?? 0, tone: st === "all" ? undefined : stateTone[st], title: `${STATE_HINT[st]} (${i + 1})` })));
  const levelOpts = $derived(levelKeys.map((l) => ({ key: l, label: l, count: levelN[l] ?? 0 })));
  // the modules table sorts here; the requirement, test and spec lists sort inside DataTable
  let modSort = $state<Sort>({ key: "", dir: 1 });
  const reqVal = (r: Row, k: string) => (k === "file" ? `${r.file}:${String(r.line).padStart(6, "0")}` : (r as any)[k]);
  const modVal = (m: any, k: string) => (k === "languages" ? m.languages.join(", ") : k === "symbols" ? symbolsOf(m) : k === "depends" ? m.depends_on.length : (m[k] ?? ""));
  const prefixOf = (id: string) => id.replace(/-.*$/, "");
  // requirement table columns (the state colours the row edge)
  const reqCols = [{ key: "id", label: "ID", cls: "mono nowrap" }, { key: "level", label: "level", cls: "nowrap" }, { key: "state", label: "state" }, { key: "text", label: "requirement" }, { key: "file", label: "location", cls: "mono muted nowrap" }];
  const invalidN = $derived(report ? Object.values(report.invalid_tests).reduce((n: number, v: any) => n + v.length, 0) : 0);
  const known = $derived(new Set(rows.map((r) => r.id)));
  const mods = $derived<any[]>(sorted((modules?.modules ?? []).filter((m: any) => (!onlyChanged || changedMods.has(m.path)) && (!query || m.path.toLowerCase().includes(query.toLowerCase()))), modSort, modVal));
  const hiddenMods = $derived<string[]>((modules?.hidden ?? []).filter((m: string) => !query || m.toLowerCase().includes(query.toLowerCase())));
  const modOf = $derived(modules?.modules.find((m: any) => m.path === route.module));
  const usedBy = $derived((modules?.modules ?? []).filter((m: any) => modOf && m.depends_on.some((d: any) => d.module === modOf.path)));
  const testsOf = (path: string) => tests?.modules.find((m: any) => m.module === path);
  const symbolsOf = (m: any) => Object.values<number>(m.symbols).reduce((a, b) => a + b, 0);
  // The tests screen filters the detected cases; the requirement table uses the same cases for its links (WEB-39).
  const testOpts = $derived(["all", "tagged", "untagged"].map((k) => ({ key: k, label: k, count: tcounts[k] })));
  const VIEWS = [{ key: "list", label: "📋 list" }, { key: "graph", label: "🕸️ graph" }];
  const detectedCases = $derived<any[]>((tests?.modules ?? []).flatMap((m: any) => m.files.flatMap((f: any) => f.tests.map((t: any) => ({ module: m.module, file: f.file, t })))));
  const testedIds = $derived(new Set<string>(detectedCases.flatMap((c) => c.t.ids)));
  const testRequirement = $derived(route.requirementId?.toUpperCase());
  const allCases = $derived(detectedCases.filter((c) => (!route.module || c.module === route.module) && (!testRequirement || c.t.ids.includes(testRequirement))));
  const tcounts = $derived({ all: allCases.length, tagged: allCases.filter((c) => c.t.ids.length).length, untagged: allCases.filter((c) => !c.t.ids.length).length } as Record<string, number>);
  const matched = $derived(allCases.filter((c) => (stateFilter === "all" || (stateFilter === "tagged") === c.t.ids.length > 0) && (!onlyChanged || fileOf.has(c.file)) && (!query || `${c.file} ${c.t.name} ${c.t.ids.join(" ")}`.toLowerCase().includes(query.toLowerCase()))));
  // list rows: a test is told apart by file and line, which is also how the URL selects it (`path` + `line`)
  const testRows = $derived(matched.map((c) => ({ ...c, key: `${c.file}:${c.t.line}` })));
  const testCols = [{ key: "name", label: "test", cls: "wrap" }, { key: "ids", label: "requirements", cls: "nowrap" }, { key: "file", label: "location", cls: "mono muted nowrap" }];
  const testVal = (c: any, k: string) => (k === "file" ? `${c.file}:${String(c.t.line).padStart(6, "0")}` : k === "ids" ? c.t.ids.join(" ") : c.t.name);
  const selTest = $derived(route.tab === "tests" && route.path ? `${route.path}:${route.line}` : undefined);
  const byId = $derived(new Map(rows.map((r) => [r.id, r])));
  // the preview shows the lines of the selected test: a few before it and the start of its body
  const TEST_BEFORE = 3, TEST_AFTER = 24;
  const specRows = $derived((specFiles ?? []).filter((file) => !onlyChanged || fileOf.has(file)).map((file) => ({ file })));
  // the Changes page: files grouped by state, and the counts that link to the filtered views
  const view = $derived(defaultScope ? changes : custom);
  // (WEB-33) the short hash of the explained commit, for the section titles
  const at = $derived(scope.commit ? ` (${history?.commits.find((c: any) => c.sha === scope.commit || c.short === scope.commit)?.short ?? scope.commit.slice(0, 7)})` : "");
  const CHANGES = ["added", "modified", "renamed", "deleted"];
  const nOf = (kind: string) => (view?.files ?? []).filter((f: any) => f.kind === kind).length;
  const viewMods = $derived(new Set<string>((view?.files ?? []).map((f: any) => moduleOfFile(f.path))));
  const noTestN = $derived(directReqs.filter((r: Row) => r.state === "none" || r.state === "src-only").length);
  // one pill per kind of note, the files it happened in as its title
  const noteGroups = (notes: { role: string; action: string; file: string }[]) => {
    const g = new Map<string, Set<string>>();
    for (const n of notes) g.set(`${n.role} ${n.action}`, (g.get(`${n.role} ${n.action}`) ?? new Set()).add(n.file));
    return [...g].map(([k, v]) => [k, [...v]] as const);
  };
  const tallyBy = (files: any[]) => CHANGES.map((c) => [c, files.filter((f) => f.change === c).length] as const).filter(([, n]) => n);
  const taggedN = $derived(tests ? tests.modules.reduce((n: number, m: any) => n + m.files.reduce((k: number, f: any) => k + f.tests.filter((t: any) => t.ids.length).length, 0), 0) : 0);
  const plain = (t: string) => t.replace(/[`*~]/g, "");
  const req = (id: string): Route => ({ tab: "requirements", id });
  const withId = (tab: Route["tab"], id: string): Route => ({ tab, id });
  // typing in the filter replaces the URL instead of stacking a history entry per keystroke
  const filter = (patch: Partial<Route>) => navigate({ ...route, ...patch }, true);
  // highlighted line HTML with known requirement IDs turned into links (only text between tags is touched)
  const linkIds = (html: string) => html.replace(/(<[^>]+>)|\b[A-Z]{2,5}-\d{1,4}\b/g, (m, tag) => (tag || !known.has(m) ? m : `<a class="id" href="${u(req(m))}">${m}</a>`));
  const lines = $derived(code ? highlightLines(code.path, code.text).map(linkIds) : []);

  // code viewer: wrap, copy the path, open in an editor (URL scheme of the chosen IDE, remembered per browser)
  // `scheme://file/<abs path>:<line>` (VS Code family, Zed), JetBrains `open?file=&line=`, or the editor's own form
  const fileUrl = (scheme: string) => (f: string, l: number) => `${scheme}://file${f}:${l}`;
  const jetbrains = (scheme: string) => (f: string, l: number) => `${scheme}://open?file=${encodeURIComponent(f)}&line=${l}`;
  const EDITORS: Record<string, (file: string, line: number) => string> = {
    IntelliJ: jetbrains("idea"),
    WebStorm: jetbrains("webstorm"),
    PyCharm: jetbrains("pycharm"),
    GoLand: jetbrains("goland"),
    RustRover: jetbrains("rustrover"),
    CLion: jetbrains("clion"),
    PhpStorm: jetbrains("phpstorm"),
    Rider: jetbrains("rider"),
    "Android Studio": jetbrains("androidstudio"),
    "VS Code": fileUrl("vscode"),
    "VS Code Insiders": fileUrl("vscode-insiders"),
    VSCodium: fileUrl("vscodium"),
    Cursor: fileUrl("cursor"),
    Windsurf: fileUrl("windsurf"),
    Zed: fileUrl("zed"),
    Sublime: (f, l) => `subl://open?url=${encodeURIComponent("file://" + f)}&line=${l}`,
    TextMate: (f, l) => `txmt://open?url=${encodeURIComponent("file://" + f)}&line=${l}`,
    Xcode: (f, l) => `xcode://open?file=${encodeURIComponent(f)}&line=${l}`,
  };
  const savedEditor = stored("spec17.editor");
  let editor = $state(savedEditor in EDITORS ? savedEditor : "IntelliJ");
  let wrap = $state(stored("spec17.wrap") === "1");
  let copied = $state(false);
  let showHidden = $state(stored("spec17.hidden") === "1");
  let modView = $state(stored("spec17.modules") === "graph" ? "graph" : "list");
  const save = (k: string, v: string) => { try { localStorage.setItem(k, v); } catch { /* storage blocked */ } };
  const copyPath = (path: string) => navigator.clipboard.writeText(path).then(() => { copied = true; setTimeout(() => (copied = false), 1200); }).catch(() => {});
</script>

{#snippet reqCell(r: any, k: string)}
  {#if k === "id"}{#if commits[r.id]}<input type="checkbox" class="rv" aria-label="reviewed" title={reviewTip(r.id)} checked={isReviewed(r.id)} onchange={(e) => setReviewed(r.id, e.currentTarget.checked ? commits[r.id].sha : undefined)} />{:else if Object.keys(commits).length}<span class="rv muted" title="not committed yet: commit it to review it">·</span>{/if}{#if r.deleted || !known.has(r.id)}{r.id}{:else}<a href={u(route.tab === "requirements" ? { ...route, id: r.id } : req(r.id))}>{r.id}</a>{/if}
  {:else if k === "level"}{#if /^(MUST|SHOULD|MAY)$/.test(r.level)}<span class="rfc {r.level.toLowerCase()}">{r.level}</span>{/if}
  {:else if k === "state"}<Pill tone={r.deleted ? "bad" : stateTone[r.state]}>{r.state}</Pill>{#if r.done}<Pill tone="accent" title="check-marked: {r.verdict}">✓ {r.verdict}</Pill>{/if}
  {:else if k === "text"}<div class="txt" title={plain(r.text)}>{@html inline(r.text)}</div>
    <div class="notes">
      {#if !r.deleted && testedIds.has(r.id)}
        <a href={u({ tab: "tests", requirementId: r.id })} aria-label="View tagged tests for {r.id}">View tagged tests</a>
      {/if}
      {#each noteGroups(r.notes ?? changeOf.get(r.id)?.notes ?? []) as [what, files]}<Pill tone={r.deleted ? "bad" : "accent"} title={files.join(", ")}>✎ {what}</Pill>{/each}
    </div>
  {:else if r.line}<a href={u({ tab: "specs", file: r.file, id: r.id })}>{r.file}:{r.line}</a>{:else}{r.file}{/if}
{/snippet}

{#snippet who(c: any, note = false)}
  {#if c && note}<p class="muted note">Last changed by <b>{c.author}</b>, {c.date} · <span class="mono">{c.short}</span> {c.subject}</p>
  {:else if c}<span class="muted who" title={c.subject}>{c.author}, {c.date} · <span class="mono">{c.short}</span></span>{/if}
{/snippet}

{#snippet idlink(id: string, to: Route)}
  <IdLink href={u(to)}>{id}</IdLink>
{/snippet}

{#snippet th(label: string, key: string, st: Sort, cls = "")}
  <th class={cls} aria-sort={st.key === key ? (st.dir === 1 ? "ascending" : "descending") : "none"}>
    <button type="button" class="sort" onclick={() => toggleSort(st, key)}>{label}<span class="arrow">{st.key === key ? (st.dir === 1 ? "▲" : "▼") : ""}</span></button>
  </th>
{/snippet}

{#snippet cases(files: any[])}
  {#each files as f}
    <h3 class="file mono nowrap"><a href={u({ tab: "code", path: f.file })}>{f.file}</a> <span class="muted">{f.tests.length}</span></h3>
    <ul class="refs">
      {#each f.tests as t}
        <li><a class="mono" href={u({ tab: "code", path: f.file, line: t.line })} title={t.qualified}>{t.name}</a> {#each t.ids as id}{@render idlink(id, known.has(id) ? req(id) : { tab: "requirements", q: id })}{" "}{/each}{#if !t.ids.length}<span class="muted">untagged</span>{/if}</li>
      {/each}
    </ul>
  {/each}
{/snippet}

{#snippet tracePanel(float = false)}
  {#if trace || !float}
    <aside class:float class:stale={trace && route.id && trace.data?.id?.toUpperCase() !== route.id.toUpperCase()}>
      {#if !trace || !route.id}
        <p class="muted empty">Select a requirement to preview it.</p>
      {:else}
      {@const sp = trace.data?.spec}
      <PaneHead title={trace.data.id} close={u({ ...route, id: undefined })}>{#if sp}<Pill>{sp.level}</Pill>{/if}</PaneHead>
      {@const dfiles = reqFiles(trace.data, reqKinds.get(trace.data.id))}
      {#if dfiles.length}<p><ViewSwitch n={dfiles.length} /></p>{/if}
      {#if diffOn(dfiles.length > 0)}
        <DiffView paths={dfiles} {gen} href={(p) => u({ tab: "code", path: p })} />
        <p class="muted note">What changed against HEAD in the files that spec, tag or cite this requirement. Evidence of edits, not that it is done.</p>
      {:else}
      {#if sp}
        <p class="muted mono nowrap"><a href={u({ tab: "specs", file: sp.file, id: trace.data.id })}>{sp.file}:{sp.line}</a></p>
        {@render who(commits[trace.data.id], true)}
        <Markdown source={sp.text} />
      {:else}<p class="muted">Not in any spec: cited only in tests or code.</p>{/if}
      {#if trace.data.tests?.length}
        <h4>Tests ({trace.data.tests.length})</h4>
        <ul class="refs">{#each trace.data.tests as t}<li><a class="mono nowrap" href={u({ tab: "code", path: t.file, line: t.line })}>{t.file}:{t.line}</a> {t.text} {@render who(blame[blameKey(t.file, t.line)])}</li>{/each}</ul>
      {/if}
      {#if trace.data.source_cites?.length}
        <h4>Code citations ({trace.data.source_cites.length})</h4>
        <ul class="refs">{#each trace.data.source_cites as c}<li><a class="mono nowrap" href={u({ tab: "code", path: c.file, line: c.line })}>{c.file}:{c.line}</a> <code>{c.decl}</code></li>{/each}</ul>
      {/if}
      {#if trace.data.related?.length}
        <h4>Related code ({trace.data.related_total})</h4>
        <ul class="refs">{#each trace.data.related as r}<li><a class="mono nowrap" href={u({ tab: "code", path: r.file })}>{r.file}</a> <span class="muted">{r.types.join(", ")}{r.cites ? " · cites it" : ""}</span></li>{/each}</ul>
      {/if}
      <details class="plain"><summary class="muted">plain text</summary><pre>{trace.text}</pre></details>
      <p class="muted note">Evidence, not truth.</p>
      {/if}
      {/if}
    </aside>
  {/if}
{/snippet}

{#snippet testPanel()}
  {#if !route.path}
    <p class="muted empty">Select a test to preview it.</p>
  {:else}
    {@const c = allCases.find((x) => x.file === route.path && x.t.line === route.line)}
    {@const lo = Math.max(1, (route.line ?? 1) - TEST_BEFORE)}
    <PaneHead title={c?.t.name ?? route.path} close={u({ ...route, path: undefined, line: undefined })} />
    <p class="muted mono nowrap"><a href={u({ tab: "code", path: route.path, line: route.line })}>{route.path}:{route.line}</a>{#if c}{" "}in <a href={u({ tab: "modules", module: c.module })}>{c.module}</a>{/if}</p>
    {@render who(route.path && route.line ? blame[blameKey(route.path, route.line)] : undefined, true)}
    {#if fileOf.has(route.path)}<p><ViewSwitch /></p>{/if}
    {#if diffOn(fileOf.has(route.path))}
      <DiffView paths={[route.path]} {gen} />
    {:else}
    {#if c}
      <h4>Requirements</h4>
      {#if c.t.ids.length}
        <ul class="refs">{#each c.t.ids as id}<li>{@render idlink(id, known.has(id) ? req(id) : { tab: "requirements", q: id })} <span class="muted">{byId.get(id) ? plain(byId.get(id)!.text).slice(0, 140) : "not in any spec"}</span></li>{/each}</ul>
      {:else}<p class="muted">Untagged: no requirement ID in its tag.</p>{/if}
    {/if}
    <h4>Source</h4>
    {#if code?.path === route.path}
      <table class="code snip">
        <tbody>
          {#each lines.slice(lo - 1, lo - 1 + TEST_BEFORE + TEST_AFTER + 1) as l, i}
            <tr class:hl={route.line === lo + i}>
              <td class="ln"><a href={u({ tab: "code", path: route.path, line: lo + i })}>{lo + i}</a></td>
              <td class="src">{@html l}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="muted note"><a href={u({ tab: "code", path: route.path, line: route.line })}>Open the whole file</a>. A tag is evidence, not proof that the test passes.</p>
    {:else}<p class="muted">Loading {route.path}…</p>{/if}
    {/if}
  {/if}
{/snippet}

<header>
  <h1><a href="/">spec17</a></h1>
  <span class="dot" data-tone={up === true ? "ok" : up === false ? "bad" : undefined} role="status" title={up === null ? "server: connecting" : up ? "server: connected" : "server: disconnected, retrying"}></span>
  <a class="root mono muted" href="/">{root}</a>
  <nav aria-label="views">
    {#each TABS.filter((t) => t !== "setup") as t}
      <a class="tab" class:on={route.tab === t} aria-current={route.tab === t ? "page" : undefined} href={u({ tab: t })} title={t} aria-label={t}>
        <span aria-hidden="true">{EMOJI[t]}</span><span class="lbl">{t}</span>{#if t === "dashboard" && report?.total_problems}<span class="badge" data-tone="bad" title="{report.total_problems} problems">{report.total_problems}</span>{/if}{#if t === "changes" && directReqs.length}<span class="badge" data-tone="accent" title="{directReqs.length} requirements may be affected">{changes.requirements.length}</span>{/if}
      </a>
    {/each}
  </nav>
  <button class="icon" disabled={busy} onclick={rescan} title="Rescan the repo (syncs the CodeGraph index first)" aria-label="rescan">↻</button>
  <button class="icon" onclick={() => (theme = nextTheme.v)} title="Theme: {curTheme.name} (click for {nextTheme.name})" aria-label="Theme: {curTheme.name}, switch to {nextTheme.name}">{curTheme.icon}</button>
  <a class="tab icon" class:on={route.tab === "setup"} aria-current={route.tab === "setup" ? "page" : undefined} href={u({ tab: "setup" })} title={setup?.suggestions.length ? "Setup (not fully set up)" : "Setup"} aria-label="Setup">⚙️{#if setup?.suggestions.length}<span class="badge" data-tone="warn" title="not fully set up">!</span>{/if}</a>
</header>

{#if error}<p class="error">{error}</p>{/if}
{#if setup && setup.suggestions.length && route.tab !== "setup" && dismissed !== setup.suggestions[0]}
  <p class="banner">
    <span>spec17 is not fully set up for this repo: {setup.suggestions[0]}</span>
    <span class="grow"></span>
    <a class="tab" href="/setup">Open Setup</a>
    <button aria-label="dismiss" title="Hide until the message changes" onclick={() => dismiss(setup!.suggestions[0])}>✕</button>
  </p>
{/if}

<main>
  {#if route.tab === "setup"}
    {#if !setup}
      {#if setupError}<p class="error">Could not check the setup: {setupError} <button onclick={loadSetup}>Retry</button></p>{:else}<p class="muted">Checking…</p>{/if}
    {:else}
      <!-- (WEB-42) -->
      {#if setup.version}<p class="setup-value muted">spec17 <code>{setup.version}</code></p>{/if}
      {#snippet mcpControls(transport: "stdio" | "http", state: string, path: string, global: boolean, details?: string | null)}
        {@const label = transport === "stdio" ? "stdio MCP" : "HTTP MCP"}
        {@const present = state === "ok" || state === "other"}
        <p class="setup-actions">
          <Pill tone={state === "ok" ? "ok" : state === "other" ? "warn" : "bad"}>{label} {state === "ok" ? "installed" : state === "other" ? "custom entry" : "missing"}</Pill>
          {#if present}
            <button disabled={busy} onclick={() => confirm(`Remove the ${label} entry from ${path}?`) && runSetup("uninstall", global, { only: "mcp", transport })}>Remove {label}</button>
          {:else}
            <button disabled={busy} onclick={() => runSetup("install", global, { only: "mcp", transport })}>Install {label}</button>
          {/if}
        </p>
        {#if present && details}<p class="setup-value muted"><code>{details}</code></p>{/if}
      {/snippet}
      {#snippet row(title: string, where: any, global: boolean)}
        <div class="card block setup">
          <h3>{title}</h3>
          <!-- (WEB-38, WEB-40) -->
          <p class="setup-actions">
            <Pill tone={where.skill === "current" ? "ok" : where.skill === "outdated" ? "warn" : "bad"}>skill {where.skill}</Pill>
            <button disabled={busy || where.skill === "current"} onclick={() => runSetup("install", global, { only: "skill" })}>{where.skill === "outdated" ? "Update skill" : "Install skill"}</button>
            <button disabled={busy || where.skill === "missing"} onclick={() => confirm(`Remove the spec17 skill at ${where.skill_path}?`) && runSetup("uninstall", global, { only: "skill" })}>Remove skill</button>
          </p>
          <p class="setup-value muted"><code>{where.skill_path}</code></p>
          {#if where.mcp_stdio !== undefined}
            <p class="setup-value setup-config muted"><code>{where.mcp_path}</code></p>
            <!-- (WEB-41) -->
            {@render mcpControls("stdio", where.mcp_stdio, where.mcp_path, global, launchCommand(where.mcp_details?.stdio))}
            {@render mcpControls("http", global ? where.mcp_file : where.mcp, where.mcp_path, global, where.mcp_details?.http_url)}
          {:else}
            <p class="muted">Update spec17 and restart the server to manage MCP transports.</p>
          {/if}
          {#if global && where.mcp !== "missing"}
            <p class="muted">Claude user MCP entry {where.mcp === "ok" ? "installed" : "customised"} · <code>~/.claude.json</code> (read-only)</p>
          {/if}
        </div>
      {/snippet}
      {#if noReqs}
        <section class="card block">
          <h3>No specs yet</h3>
          <p>spec17 found no requirements in this repo and works without them: Modules and Tests still list its sources and tests. {setup.suggestions.length ? "Install the skill below, then ask" : "Ask"} your agent to draft <code>specs/01-name.md</code> with you (lines like <code>- **ABC-12** MUST — text</code>).</p>
        </section>
      {/if}
      {@render row("This repo", setup.status.project, false)}
      {@render row("Global", setup.status.global, true)}
      <p class="muted">Stdio MCP starts a spec17 process for each session. HTTP MCP connects to a running <code>spec17</code> server.</p>
      {@const cgBin = checks.find((k) => k.name === "codegraph binary")}
      {@const cgIdx = checks.find((k) => k.name === "codegraph index")}
      {@const cgMcp = setup.status.project.codegraph_mcp === "ok" ? "this repo" : setup.status.global.codegraph_mcp === "ok" ? "global" : ""}
      <div class="card block setup">
        <h3>CodeGraph <small class="muted">optional: test-to-code links, overlap, uncovered code</small></h3>
        <p><b>1. Install</b> <Pill tone={cgBin?.level === "ok" ? "ok" : "warn"}>{cgBin?.level === "ok" ? `installed ${cgBin.detail}` : "not on PATH"}</Pill></p>
        {#if cgBin && cgBin.level !== "ok"}
          <pre class="log">curl -fsSL https://raw.githubusercontent.com/colbymchenry/codegraph/main/install.sh | sh
npm i -g @colbymchenry/codegraph
irm https://raw.githubusercontent.com/colbymchenry/codegraph/main/install.ps1 | iex   # Windows PowerShell</pre>
          <p class="muted">Pick one, then press ↻ below. <a href="https://colbymchenry.github.io/codegraph/getting-started/installation/" target="_blank" rel="noreferrer">Installation docs</a>. spec17 never installs it for you.</p>
        {/if}
        <p>
          <b>2. Connect the agent</b> <Pill tone={cgMcp ? "ok" : "warn"}>{cgMcp ? `codegraph server set (${cgMcp})` : "codegraph server missing"}</Pill>
          <button disabled={busy || setup.status.project.codegraph_mcp === "ok"} onclick={() => runSetup("install", false, { server: "codegraph" })} title="Adds the codegraph MCP server to .mcp.json (created if missing; an entry that is there is kept)">Add CodeGraph to .mcp.json</button>
          <button disabled={busy || setup.status.project.codegraph_mcp !== "ok"} onclick={() => confirm("Remove the codegraph server from .mcp.json? Your agent loses CodeGraph until it is added again, and a customised entry is not restored.") && runSetup("uninstall", false, { server: "codegraph" })} title="Removes only the codegraph entry from .mcp.json; the skill and the spec17 entry stay">Remove</button>
        </p>
        <p><b>3. Index</b> {#if cgIdx}<Pill tone={levelTone(cgIdx.level)}>{cgIdx.level}</Pill> <span class="muted">{cgIdx.detail}</span>{/if}</p>
        {#if cgBin?.level === "ok"}
          <p>
            {#if indexAction === "init"}<button class="primary" disabled={busy} onclick={() => runIndex("init")}>Create the index (codegraph init)</button>
            {:else if cgIdx?.level === "ok" || indexAction === "sync"}<button disabled={busy} onclick={() => runIndex("sync")}>Sync now (codegraph sync)</button>{/if}
          </p>
        {/if}
      </div>
      <h3>Checks <button onclick={loadSetup}>↻</button></h3>
      <table>
        <tbody>
          {#each checks as k}
            <tr>
              <td class="nowrap"><Pill tone={levelTone(k.level)}>{k.level}</Pill></td>
              <td class="nowrap">{k.name}</td>
              <td>
                {k.detail}{#if k.hint}<div class="muted">→ {k.hint}</div>{/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if indexLog}<pre class="log">{indexLog}</pre>{/if}
    {/if}
  {:else if !report}
    {#if error}<p class="muted">The scan failed (see above). <a href="/setup">Setup</a> shows what is missing.</p>{:else}<p class="muted">Scanning…</p>{/if}
  {:else if route.tab === "dashboard"}
    {#if noReqs}
      <section class="hero card">
        <h2>No requirements yet</h2>
        <p>spec17 found none. Write them as <code>- **ABC-12** MUST — text</code> in <code>specs/01-name.md</code> or <code>SPEC.md</code>, tag tests <code>[ABC-12]</code> and cite <code>// (ABC-12)</code> in code. <a href="/setup">Setup</a> installs the skill so your agent drafts them with you.</p>
        {#if modules}<p class="facts muted"><a href={u({ tab: "modules" })}>{plural(sum("src_files"), "source file")}</a> · {plural(sum("test_files"), "test file")} · <a href={u({ tab: "tests" })}>{plural(sum("tests"), "test")} detected</a></p>{/if}
      </section>
    {:else}
    <section class="hero card">
      <div class="pct"><b>{report.tested_pct}%</b><span>of due requirements have a tagged test <small class="muted">({report.tested}/{report.due})</small></span></div>
      <div class="meter" role="img" aria-label="{report.tested_pct}% tested"><i style="width:{report.tested_pct}%"></i></div>
      <p class="facts muted">
        <a href={u({ tab: "requirements" })}>{report.requirements} requirements</a> in {report.spec_files} specs
        {#if report.manual} · <a href={u({ tab: "requirements", state: "manual" })}>{report.manual} manual</a>{/if}
        {#if report.later} · <a href={u({ tab: "requirements", state: "later" })}>{report.later} later</a>{/if}
        {#if report.refs} · <a href={u({ tab: "requirements", state: "ref" })}>{report.refs} reference docs</a>{/if}
        · <b data-tone={report.total_problems ? "bad" : "ok"}>{report.total_problems ? `${report.total_problems} problems` : "no problems found"}</b>
      </p>
    </section>
    {/if}
    {#if !report.index}<p class="note-line muted">No CodeGraph index: test-to-code links and uncovered code are not checked.</p>{/if}
    {#if changes}
      <Section title="Changes" count={changes.files.length} tone={changes.files.length ? "accent" : undefined}>
        {#if !changes.files.length}<p class="muted">Working tree clean: nothing to compare with HEAD.</p>
        {:else}
          <p>{#each tallyBy(changes.files) as [c, n]}<Pill tone={changeTone(c)}>{n} {c}</Pill>{/each}</p>
          <p>
            <a href={u({ tab: "requirements", changed: "1" })}>{plural(directReqs.length, "requirement")}</a> may be affected{#if noTestN} · <b data-tone="bad">{noTestN} without a test</b>{/if}
            <span class="muted"> · evidence of edits, not of done</span>
          </p>
        {/if}
        <p><a href={u({ tab: "changes" })}>All changes</a></p>
      </Section>
    {/if}
    {#if modules?.modules.length}
      <Section title="Modules" count={modules.modules.length}>
        <table>
          <tbody>
            {#each [...modules.modules].sort((a, b) => b.tests - a.tests || a.path.localeCompare(b.path)).slice(0, 8) as m}
              <tr>
                <td class="mono nowrap"><a href={u({ tab: "modules", module: m.path })}>{m.path}</a></td>
                <td class="muted">{m.languages.join(", ")}</td>
                <td class="nowrap">{m.tests} tests</td>
                <td class="muted nowrap">{m.src_files + m.test_files} files</td>
                <td class="muted">{#if m.depends_on.length}→ {m.depends_on.slice(0, 3).map((d: any) => d.module).join(", ")}{/if}</td>
              </tr>
            {/each}
          </tbody>
        </table>
        <p><a href="/modules">All {modules.modules.length} modules</a></p>
      </Section>
    {/if}

    {#snippet ids(title: string, list: string[], sev: Tone)}
      {#if list.length}
        <Section {title} count={list.length} tone={sev}>
          <p class="ids">{#each list as id}{@render idlink(id, req(id))}{/each}</p>
        </Section>
      {/if}
    {/snippet}
    {@render ids("Unimplemented: due, no test and no code", report.unimplemented, "bad")}
    {@render ids("Untested: due, cited in code but no test", report.untested, "warn")}
    {@render ids("Marked later but already has tests or code", report.built, "warn")}

    {#if report.unproven.length}
      <Section title="Check-marked done without proof" count={report.unproven.length} tone="warn">
        <ul>{#each report.unproven as x}<li>{@render idlink(x.id, req(x.id))} <code>{x.verdict}</code> <a href={u({ tab: "specs", file: x.file, id: x.id })}>{x.file}:{x.line}</a> <span class="muted">{plain(x.text).slice(0, 80)}</span></li>{/each}</ul>
      </Section>
    {/if}
    {#if invalidN}
      <Section title="Invalid tests" count={invalidN} tone="bad">
        {#each Object.entries(report.invalid_tests) as [kind, list]}
          {#if (list as any[]).length}<p><code>{kind}</code> {#each list as any[] as p}{@render idlink(p.id, req(p.id))} <a class="muted mono" href={u({ tab: "code", path: p.file, line: p.line })}>{p.file}:{p.line}</a> {/each}</p>{/if}
        {/each}
      </Section>
    {/if}
    {#if report.spec_issues.length}
      <Section title="Spec issues" count={report.spec_issues.length} tone="bad">
        <ul>{#each report.spec_issues as x}<li class="mono">{x}</li>{/each}</ul>
      </Section>
    {/if}
    {#if report.dangling_tests.length + report.dangling_sources.length}
      <Section title="Dangling citations" count={report.dangling_tests.length + report.dangling_sources.length} tone="warn">
        <p class="mono">{#if report.dangling_tests.length}tests: {report.dangling_tests.join(", ")}<br />{/if}{#if report.dangling_sources.length}sources: {report.dangling_sources.join(", ")}{/if}</p>
      </Section>
    {/if}
  {:else if route.tab === "requirements" && noReqs}
    <p class="muted empty">No requirements found. <a href="/setup">Setup</a> says what spec17 looks for.</p>
  {:else if route.tab === "requirements"}
    <div class="page">
    <div class="bar">
      <Seg label="State" keycap="S" hint="Press 1–7 to pick a state, S for the next one, ⇧S for the previous one" options={stateOpts} value={stateFilter} onpick={(k) => filter({ state: k })} />
      <Seg label="Level" keycap="L" hint="Press L for the next level, ⇧L for the previous one" options={levelOpts} value={levelFilter || "all"} onpick={(k) => filter({ level: k === "all" ? undefined : k })} />
      {#if showChanged}<Seg label="Changes" hint="Only what the working tree changed against HEAD" options={changedSeg(rows.filter((r) => reqKinds.has(r.id)).length)} value={onlyChanged ? "changed" : "all"} onpick={pickChanged} />{/if}
      {#if Object.keys(commits).length}<Seg label="Reviewed" hint="Hide the requirements you marked reviewed (and have not changed since)" options={[{ key: "all", label: "all" }, { key: "unreviewed", label: "unreviewed", count: rows.filter((r) => !isReviewed(r.id)).length }]} value={onlyUnreviewed ? "unreviewed" : "all"} onpick={pickReviewed} />{/if}
      <input name="filter" aria-label="filter" placeholder="Filter ID, text, file   ( / )" value={query} oninput={(e) => filter({ q: e.currentTarget.value })} />
      <span class="found muted nowrap" aria-live="polite">{#if filtering}{shown.length} of {rows.length} <a href={u({ tab: "requirements" })} title="Clear every filter (0)">Clear filters <kbd>0</kbd></a>{/if}</span>
      <SplitToggle />
    </div>
    <Split>
      {#snippet list()}
        <DataTable rows={shown} columns={reqCols} cell={reqCell} rowKey={(r) => r.id} selected={route.id} onselect={(k, how) => navigate({ ...route, id: k }, how === "key")} value={reqVal} groupOf={(r) => prefixOf(r.id)} tone={(r) => edgeTone(r.state)} pale={(r) => isReviewed(r.id)} storageKey="spec17.collapsed">
          {#snippet empty()}<p class="muted empty">No requirement matches. <a href={u({ tab: "requirements" })}>Clear filters</a></p>{/snippet}
        </DataTable>
      {/snippet}
      {#snippet preview()}{@render tracePanel()}{/snippet}
    </Split>
    </div>
  {:else if route.tab === "code"}
    {#if !route.path}<p class="muted">No file selected: open one from a requirement's tests or citations.</p>
    {:else if !code || code.path !== route.path}<p class="muted">Loading {route.path}…</p>
    {:else}
      <p class="mono crumb">
        <span class="nowrap"><a href="/requirements">requirements</a> / {code.path}{route.line ? `:${route.line}` : ""}</span>
        <button class="small" title="Copy the file path" onclick={() => copyPath(code!.path)}>{copied ? "Copied" : "Copy path"}</button>
        {#if root}<a class="btn small" title="Open in {editor}" href={EDITORS[editor](`${root}/${code.path}`, route.line ?? 1)}>Open in</a>{/if}
        <select class="small" bind:value={editor} onchange={() => save("spec17.editor", editor)} aria-label="Editor">{#each Object.keys(EDITORS) as e}<option>{e}</option>{/each}</select>
        <button class="small" class:on={wrap} aria-pressed={wrap} onclick={() => { wrap = !wrap; save("spec17.wrap", wrap ? "1" : "0"); }}>Wrap</button>
        {#if fileOf.has(code.path)}<ViewSwitch />{/if}
      </p>
      {#if diffOn(fileOf.has(code.path))}<DiffView paths={[code.path]} {gen} />
      {:else}
      <table class="code" class:wrap>
        <tbody>
          {#each lines as l, i}
            <tr id={`L${i + 1}`} class:hl={route.line === i + 1}>
              <td class="ln"><a href={u({ tab: "code", path: code.path, line: i + 1 })}>{i + 1}</a></td>
              <td class="src">{@html l}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      {/if}
    {/if}
  {:else if route.tab === "modules" && route.module}
    <p class="mono nowrap crumb"><a href="/modules">modules</a> / {route.module}</p>
    {#if !modules}<p class="muted">Loading…</p>
    {:else if !modOf}<p class="muted empty">No module <code>{route.module}</code>. <a href="/modules">All modules</a></p>
    {:else}
      <section class="card block">
        <h2 class="mono">{modOf.path}</h2>
        <p>
          {#if modOf.manifest}<Pill>{modOf.manifest}</Pill>{/if}
          {#each modOf.languages as l}<Pill>{l}</Pill>{/each}
          <span class="muted">{plural(modOf.src_files, "source file")} · {plural(modOf.test_files, "test file")} · {plural(symbolsOf(modOf), "symbol")}</span>
        </p>
        {#if Object.keys(modOf.symbols).length}<p class="muted">{Object.entries(modOf.symbols).map(([k, n]) => `${n} ${k}`).join(" · ")}</p>{/if}
        {#if modOf.depends_on.length}<p>depends on {#each modOf.depends_on as d}<IdLink href={u({ tab: "modules", module: d.module })}>{d.module}</IdLink>{" "}{/each}</p>{/if}
        {#if usedBy.length}<p>used by {#each usedBy as d}<IdLink href={u({ tab: "modules", module: d.path })}>{d.path}</IdLink>{" "}{/each}</p>{/if}
      </section>
      <h2>Sources <span class="muted">{modOf.sources.length}</span></h2>
      {#if modOf.sources.length}<p class="mono">{#each modOf.sources as f}<IdLink href={u({ tab: "code", path: f })}>{f}</IdLink>{" "}{/each}</p>{:else}<p class="muted">No source files in this module.</p>{/if}
      <h2>Tests <span class="muted">{modOf.tests}</span> <a class="more" href={u({ tab: "tests", module: modOf.path })}>filter in Tests</a></h2>
      {#if tests === null}<p class="muted">Loading tests…</p>
      {:else if !testsOf(modOf.path)}<p class="muted">No tests detected in this module.</p>
      {:else}{@render cases(testsOf(modOf.path).files)}{/if}
      {#if tests}<p class="muted note">{tests.note}</p>{/if}
    {/if}
  {:else if route.tab === "modules"}
    <div class="bar">
      <input name="filter" aria-label="filter modules" placeholder="Filter modules" value={query} oninput={(e) => filter({ q: e.currentTarget.value })} />
      {#if showChanged}<Seg label="Changes" options={changedSeg(changedMods.size)} value={onlyChanged ? "changed" : "all"} onpick={pickChanged} />{/if}
      <span class="muted">{modules?.modules.length ?? 0} modules</span>
      {#if modView === "list" && modules?.hidden?.length}
        <label class="muted nowrap"><input type="checkbox" checked={showHidden} onchange={(e) => save("spec17.hidden", (showHidden = e.currentTarget.checked) ? "1" : "0")} /> show hidden ({modules.hidden.length})</label>
      {/if}
      <Seg label="View" options={VIEWS} value={modView} onpick={(k) => save("spec17.modules", (modView = k))} />
    </div>
    {#if modules && !modules.index}<p class="note-line muted">{modules.note}</p>{/if}
    {#if modView === "graph"}
      {#if !modules}<p class="muted">Loading…</p>
      {:else if !modules.index}<!-- the note above says why there is no graph -->
      {:else if !modules.modules.some((m: any) => m.depends_on.length)}<p class="muted empty">No dependencies between modules in the index.</p>
      {:else}<Graph modules={modules.modules} q={query} open={(p) => navigate({ tab: "modules", module: p })} />{/if}
    {:else if mods.length || (showHidden && hiddenMods.length)}
      <table>
        <thead><tr>{@render th("module", "path", modSort)}{@render th("manifest", "manifest", modSort)}{@render th("languages", "languages", modSort)}{@render th("source", "src_files", modSort, "num")}{@render th("test files", "test_files", modSort, "num")}{@render th("tests", "tests", modSort, "num")}{@render th("symbols", "symbols", modSort, "num")}{@render th("depends on", "depends", modSort, "num")}<th></th></tr></thead>
        <tbody>
          {#each mods as m (m.path)}
            <tr>
              <td class="mono nowrap"><a href={u({ tab: "modules", module: m.path })}>{m.path}</a>{#if changedMods.has(m.path)}{" "}<Pill tone="accent" title="files of this module changed in the working tree">✎ changed</Pill>{/if}</td>
              <td class="muted">{m.manifest ?? ""}</td>
              <td class="muted">{m.languages.join(", ")}</td>
              <td class="num">{m.src_files}</td>
              <td class="num">{m.test_files}</td>
              <td class="num"><a href={u({ tab: "tests", module: m.path })}>{m.tests}</a></td>
              <td class="num">{symbolsOf(m)}</td>
              <td class="num">{m.depends_on.length}</td>
              <td>{#if m.path !== "."}<button type="button" class="ignore" title="Add /{m.path}/ to .spec17ignore" onclick={() => setHidden(m.path, true)}>ignore</button>{/if}</td>
            </tr>
          {/each}
          {#if showHidden}
            {#each hiddenMods as path (path)}
              <tr class="hidden-row">
                <td class="mono nowrap">{path}</td>
                <td class="muted" colspan="7">hidden by .spec17ignore</td>
                <td><button type="button" class="ignore" title="Remove /{path}/ from .spec17ignore" onclick={() => setHidden(path, false)}>unhide</button></td>
              </tr>
            {/each}
          {/if}
        </tbody>
      </table>
    {:else if modules}<p class="muted empty">No module matches. <a href="/modules">Clear filter</a></p>{/if}
  {:else if route.tab === "tests"}
    <div class="page">
    <div class="bar">
      <Seg label="Tagged" options={testOpts} value={stateFilter} onpick={(k) => filter({ state: k })} />
      {#if showChanged}<Seg label="Changes" options={changedSeg(allCases.filter((c) => fileOf.has(c.file)).length)} value={onlyChanged ? "changed" : "all"} onpick={pickChanged} />{/if}
      <select aria-label="module" value={route.module ?? ""} onchange={(e) => filter({ module: e.currentTarget.value || undefined })}>
        <option value="">all modules</option>
        {#each modules?.modules ?? [] as m}<option value={m.path}>{m.path}</option>{/each}
      </select>
      <input name="filter" aria-label="filter tests" placeholder="Filter test, file, requirement ID   ( / )" value={query} oninput={(e) => filter({ q: e.currentTarget.value })} />
      {#if testRequirement}
        <a href={u({ ...route, requirementId: undefined })} aria-label="Clear requirement filter {testRequirement}">{testRequirement} ×</a>
      {/if}
      <SplitToggle />
    </div>
    {#if tests === null}<p class="muted">Loading…</p>
    {:else}
      <p class="muted found">{tests.total} tests detected in {tests.modules.length} modules, {taggedN} tagged with a requirement. {tests.note}</p>
      <Split>
        {#snippet list()}
          <DataTable rows={testRows} columns={testCols} rowKey={(c) => c.key} selected={selTest} onselect={(k, how) => { const c = testRows.find((x) => x.key === k); navigate({ ...route, path: c?.file, line: c?.t.line }, how === "key"); }} value={testVal} groupOf={(c) => c.module} storageKey="spec17.collapsed.tests">
            {#snippet cell(c, k)}
              {#if k === "name"}<span class="mono" title={c.t.qualified}>{c.t.name}</span>{#if changeMark(c.file)}{" "}<Pill tone={changeTone(changeMark(c.file))} title="its file is {changeMark(c.file)} in the working tree">✎ {changeMark(c.file)}</Pill>{/if}
              {:else if k === "file"}<a href={u({ tab: "code", path: c.file, line: c.t.line })}>{c.file}:{c.t.line}</a>
              {:else}{#each c.t.ids as id}{@render idlink(id, known.has(id) ? req(id) : { tab: "requirements", q: id })}{" "}{/each}{#if !c.t.ids.length}<span class="muted">untagged</span>{/if}{/if}
            {/snippet}
            {#snippet empty()}<p class="muted empty">No test matches. <a href="/tests">Clear filters</a></p>{/snippet}
          </DataTable>
        {/snippet}
        {#snippet preview()}{@render testPanel()}{/snippet}
      </Split>
    {/if}
    </div>
  {:else if route.tab === "specs"}
    {#if specFiles === null}<p class="muted">Loading…</p>
    {:else if !specFiles.length}<p class="muted empty">No spec files found. <a href="/setup">Setup</a> says what spec17 looks for.</p>
    {:else}
      <div class="page">
        {#if showChanged}<div class="bar"><Seg label="Changes" options={changedSeg(specFiles.filter((f) => fileOf.has(f)).length)} value={onlyChanged ? "changed" : "all"} onpick={pickChanged} /></div>{/if}
        <Split name="specs" def={22}>
          {#snippet list()}
            <DataTable headless rows={specRows} columns={[{ key: "file", label: "spec" }]} rowKey={(r) => r.file} selected={route.file ?? specFiles![0]} onselect={(k, how) => k && navigate({ tab: "specs", file: k }, how === "key")}>
              {#snippet cell(r)}<a href={u({ tab: "specs", file: r.file })}>{r.file.replace(/\.md$/, "")}</a>{#if changeMark(r.file)}{" "}<Pill tone={changeTone(changeMark(r.file))} title="{changeMark(r.file)} in the working tree">✎ {changeMark(r.file)}</Pill>{/if}{/snippet}
            </DataTable>
          {/snippet}
          {#snippet preview()}
            {@const f = route.file ?? specFiles![0]}
            {#if fileOf.has(f)}<p><ViewSwitch /></p>{/if}
            {#if diffOn(fileOf.has(f))}<DiffView paths={[f]} {gen} />{:else}<article><Markdown source={specMd} /></article>{/if}
          {/snippet}
        </Split>
      </div>
    {/if}
  {:else if route.tab === "changes"}
    <div class="bar">
      {#if history?.base && !history.on_base}
        <Seg label="Compare with" hint="What the changes are measured against" options={[{ key: "head", label: "HEAD" }, { key: "base", label: history.base }, ...(scope.commit ? [{ key: "commit", label: scope.commit.slice(0, 7) }] : [])]} value={scope.commit ? "commit" : scope.ref === history.base ? "base" : "head"} onpick={(k) => filter({ ref: k === "base" ? history.base : undefined, commit: k === "commit" ? scope.commit : undefined })} />
      {/if}
      {#if !scope.commit}
        <Seg label="Scope" hint="Staged, unstaged and untracked, or only the index" options={[{ key: "all", label: "staged + unstaged" }, { key: "staged", label: "staged only" }]} value={scope.staged ? "staged" : "all"} onpick={(k) => filter({ staged: k === "staged" ? "1" : undefined })} />
        <label class="muted nowrap"><input type="checkbox" checked={scope.include_untracked && !scope.staged} disabled={scope.staged} onchange={(e) => filter({ untracked: e.currentTarget.checked ? undefined : "0" })} /> untracked files</label>
      {/if}
      <label class="muted nowrap"><input type="checkbox" checked={scope.ignore_whitespace} onchange={(e) => filter({ ws: e.currentTarget.checked ? "1" : undefined })} /> ignore whitespace</label>
      <label class="muted nowrap" title="Files .aiignore, .spec17ignore and .agents hide"><input type="checkbox" checked={scope.show_ignored} onchange={(e) => filter({ ignored: e.currentTarget.checked ? "1" : undefined })} /> ignored files</label>
      <label class="muted nowrap" title="Files spec17 does not scan"><input type="checkbox" checked={scope.show_all} onchange={(e) => filter({ all: e.currentTarget.checked ? "1" : undefined })} /> unscanned files</label>
    </div>
    {#if history?.commits.length}
      <Section id="changes-commits" title={history.on_base ? "Recent commits" : `Commits not on ${history.base}`} count={history.commits.length} tone={history.on_base ? undefined : "accent"}>
        {#each history.commits.filter((c: any) => scope.commit && (c.sha === scope.commit || c.short === scope.commit)) as c}<p class="note"><b>Explaining</b> <span class="mono">{c.short}</span> {c.subject} <a href={u({ tab: "changes" })}>clear</a></p>{/each}
        <div class="commits"><table>
          <tbody>
            {#each history.commits as c (c.sha)}
              <tr class:sel={scope.commit === c.sha || scope.commit === c.short}>
                <td class="mono nowrap"><a href={u({ tab: "changes", commit: c.sha })} title="Explain this commit">{c.short}</a></td>
                <td class="muted nowrap">{c.date.slice(0, 10)}</td>
                <td class="muted nowrap">{c.author}</td>
                <td>{c.subject}</td>
              </tr>
            {/each}
          </tbody>
        </table></div>
        {#if history.uncommitted}<p class="muted note">{plural(history.uncommitted, "file")} uncommitted{scope.commit ? "; pick HEAD above to see them" : ""}.</p>{/if}
      </Section>
    {/if}
    {#if view === undefined}<p class="muted">Loading…</p>
    {:else if view === null}<p class="muted empty">Not checked: this is not a git work tree, it has no commits yet, or git is not available.</p>
    {:else if !view.files.length}<p class="muted empty">{view.commit ? "This commit changed no scanned file." : `Nothing ${scope.staged ? "staged" : "changed"} against ${view.reference}.`}{#if !scope.show_all} Lockfiles, CI and build files are not scanned: <a href={u({ ...route, all: "1" })}>show unscanned files</a>.{/if}</p>
    {:else}
      {@const direct = view.requirements.filter(isDirect)}
      {@const indirect = view.requirements.filter((r: any) => !isDirect(r))}
      {#snippet reqTable(rows: any[])}
        <DataTable {rows} columns={reqCols} cell={reqCell} rowKey={(r) => r.id} onselect={(id, how) => how === "click" && id && known.has(id) && navigate(req(id))} value={reqVal} tone={(r: any) => (r.deleted ? "bad" : edgeTone(r.state))} pale={(r) => isReviewed(r.id)} keys={false} />
      {/snippet}
      <Section id="changes-requirements" title={`Requirements that may be affected${at}`} count={direct.length} tone={direct.length ? "accent" : undefined}>
        {#if !direct.length}<p class="muted">No requirement's bullet, tagged test or cited code differs from {view.commit ? "its parent" : view.reference}.</p>
        {:else}{@render reqTable(direct)}{/if}
        {#if indirect.length}<details class="indirect"><summary class="muted">{indirect.length} more reached only through tests that run the changed code (not edited themselves)</summary>{@render reqTable(indirect)}</details>{/if}
        <p class="muted note">A changed file may affect a requirement; it does not say the requirement is done or its test passes. Evidence, not truth.</p>
      </Section>
      <Section id="changes-files" title={`Files${at}`} count={view.files.length} tone="accent">
        <p class="note"><button onclick={() => (openDiffs = view.files.map((f: any) => f.path))}>Expand all diffs</button> <button onclick={() => (openDiffs = [])} disabled={!openDiffs.length}>Collapse all diffs</button></p>
        <table>
          <tbody>
            {#each view.files as f (f.path)}
              <tr>
                <td class="nowrap"><Pill tone={changeTone(f.change)}>{f.change}</Pill></td>
                <td class="muted nowrap">{f.stage}</td>
                <td class="muted nowrap">{f.kind}</td>
                <td class="mono">
                  {#if f.change === "deleted" || f.kind === "other"}{f.path}
                  {:else if f.kind === "spec"}<a href={u({ tab: "specs", file: f.path })}>{f.path}</a>
                  {:else}<a href={u({ tab: "code", path: f.path })}>{f.path}</a>{/if}
                  {#if f.old_path}<span class="muted"> ← {f.old_path}</span>{/if}
                </td>
                <td class="nowrap"><button class="icon" class:on={openDiffs.includes(f.path)} aria-pressed={openDiffs.includes(f.path)} title={openDiffs.includes(f.path) ? "Hide the diff" : "Show the diff"} aria-label="{openDiffs.includes(f.path) ? 'Hide' : 'Show'} the diff of {f.path}" onclick={() => toggleDiff(f.path)}>±</button></td>
              </tr>
              {#if openDiffs.includes(f.path)}<tr class="diffrow"><td colspan="5"><button class="hidediff" onclick={() => toggleDiff(f.path)}>Hide diff ✕</button><DiffView paths={[f.path]} {gen} {scope} /></td></tr>{/if}
            {/each}
          </tbody>
        </table>
      </Section>
      <Section title="Tests, specs and modules">
        <p>
          <a href={u({ tab: "tests", changed: "1" })}>{plural(nOf("test"), "test file")}</a> ·
          <a href={u({ tab: "specs", changed: "1" })}>{plural(nOf("spec"), "spec file")}</a> ·
          <a href={u({ tab: "modules", changed: "1" })}>{plural(viewMods.size, "module")}</a>
          <span class="muted"> changed ({plural(nOf("src"), "source file")}, {plural(nOf("other"), "other file")} not scanned)</span>
        </p>
      </Section>
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
      {@render tracePanel(true)}
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
              <!-- f.file is shown relative to the app folder; the viewer needs the repo-relative path every unit carries -->
              <td class="mono"><a href={u({ tab: "code", path: f.units[0].file })}>{f.file}</a></td>
              <td>{f.lines} lines</td>
              <td class="mono muted">{#each f.units as un}<a href={u({ tab: "code", path: un.file, line: un.lo })}>{un.name}:{un.lo}-{un.hi}</a>{" "}{/each}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  {/if}
</main>

<footer class="muted">
  Evidence, not truth: tags and citations do not prove a requirement is implemented or its test passes.
  <div>Copyright &copy; 2026 Konstantin Pavlov, <a href="https://itstaff.ee">Staff OÜ</a>. All rights reserved. · <a href="https://spec17.dev" target="_blank" rel="noopener noreferrer">About</a> · <a href="https://github.com/kpavlov/spec17/issues" target="_blank" rel="noopener noreferrer">Feedback</a></div>
</footer>

<style>
  .commits { max-height: 13rem; overflow: auto; }
  tr.sel { background: color-mix(in srgb, var(--accent) 10%, transparent); }
  /* layout */
  header { position: sticky; top: 0; z-index: 2; display: flex; align-items: center; gap: var(--gap); padding: 10px var(--gutter); background: color-mix(in srgb, var(--bg) 92%, transparent); backdrop-filter: blur(8px); border-bottom: 1px solid var(--line); }
  header > :not(.root):not(nav) { flex-shrink: 0; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--tone, var(--muted)); margin-left: -4px; }
  h1 { margin: 0; font-size: 18px; letter-spacing: -0.01em; }
  nav { margin-left: auto; display: flex; gap: 6px; min-width: 0; overflow-x: auto; scrollbar-width: none; }
  nav a { flex-shrink: 0; white-space: nowrap; }
  main { padding: var(--gutter); max-width: 100%; overflow-x: clip; }
  footer { padding: 24px 16px; text-align: center; font-size: var(--text-xs); }
  .grow { flex: 1; }
  .error { margin: 12px var(--gutter); padding: 10px 14px; border: 1px solid var(--bad); background: color-mix(in srgb, var(--bad) 10%, var(--card)); color: var(--bad); border-radius: var(--radius); }
  .banner { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin: 0; padding: 8px var(--gutter); background: color-mix(in srgb, var(--warn) 13%, var(--bg)); border-bottom: 1px solid color-mix(in srgb, var(--warn) 35%, var(--line)); font-size: var(--text-sm); }
  button:disabled { opacity: 0.5; cursor: default; }

  /* header controls */
  a.tab { color: var(--fg); background: var(--raised); text-decoration: none; border-radius: var(--radius); padding: 5px 12px; text-transform: capitalize; }
  a.tab:hover { background: var(--tint-btn); color: var(--accent); }
  a.tab.on { background: var(--tint-on); color: var(--accent); font-weight: 600; }
  h1 a, a.root { color: inherit; text-decoration: none; }
  a.root { color: var(--muted); max-width: 32ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; direction: rtl; text-align: left; }
  .badge { display: inline-block; min-width: 18px; margin-left: 6px; padding: 0 5px; border-radius: 2px; font-size: 11px; font-weight: 700; line-height: 18px; text-align: center; color: var(--card); background: var(--tone); }
  a.tab.on .badge { outline: 1px solid var(--raised); }
  .icon { padding: 5px 9px; line-height: 1.2; }
  .diffrow > td { max-width: 0; }
  .hidediff { position: sticky; top: 0; float: right; z-index: 1; }
  a.tab.icon .badge { margin-left: 3px; }
  .lbl { margin-left: 7px; }
  /* narrow: icons only, except the current tab keeps its name */
  @media (max-width: 1350px) { a.tab .lbl { display: none; } a.tab.on .lbl { display: inline; } a.tab { padding: 5px 9px; } }


  /* report */
  .card { background: var(--card); border: 1px solid var(--line); border-radius: var(--radius); padding: 14px 16px; box-shadow: var(--shadow); }
  .hero { margin-bottom: 14px; }
  .pct { display: flex; align-items: baseline; gap: 12px; flex-wrap: wrap; }
  .pct b { font-size: 34px; line-height: 1.1; letter-spacing: -0.02em; }
  .meter { height: 8px; margin: 10px 0 8px; border-radius: 4px; background: var(--raised); overflow: hidden; }
  .meter i { display: block; height: 100%; background: var(--ok); border-radius: 4px; }
  .facts { margin: 0; font-size: var(--text-sm); }
  tr.hidden-row { opacity: 0.6; }
  button.ignore { padding: 1px 8px; font-size: var(--text-sm); }
  .note-line { font-size: var(--text-sm); margin: 0 0 12px; }
  b[data-tone] { color: var(--tone); }
  .ids { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 0; }

  h4, h3.file { margin: 12px 0 4px; font-size: var(--text-sm); }
  h2 { margin: 18px 0 6px; font-size: 16px; }
  details.plain { margin: 12px 0 0; padding: 6px 10px; background: var(--card); border: 1px solid var(--line); border-radius: var(--radius); }
  a.more { font-size: var(--text-sm); font-weight: 400; margin-left: 8px; }
  select { font: inherit; }


  /* requirements */
  .bar { display: flex; gap: var(--gap); align-items: center; margin-bottom: 14px; flex-wrap: wrap; }
  .bar input:not([type=checkbox]) { flex: 1; min-width: 200px; }
  .bar input[type=checkbox] { padding: 0; margin: 0; }
  .bar label { display: inline-flex; align-items: center; gap: 6px; white-space: nowrap; cursor: pointer; }

  .found { font-size: var(--text-sm); }
  .found a { margin-left: 6px; }
  /* list and preview are panes that scroll on their own: side by side (wide screens, default) or stacked (.h, and always on narrow screens) */
  aside.stale { opacity: .55; }
  .refs a, li a { text-decoration: none; }
  .refs a:hover, li a:hover { text-decoration: underline; }
  .page { display: flex; flex-direction: column; height: calc(100dvh - 100px); min-height: 440px; }
  .notes { margin-top: 3px; line-height: 1.7; }
  .txt { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  /* a trace panel outside a split (overlap) is a floating card; inside one, Split draws the card */
  aside.float { position: sticky; top: 64px; background: var(--card); border: 1px solid var(--line); border-radius: var(--radius); padding: 10px 16px 14px; overflow: auto; max-height: calc(100vh - 80px); box-shadow: var(--shadow); }
  table.snip { margin-top: 4px; }
  aside h4 { margin: 14px 0 4px; font-size: var(--text-sm); }
  .refs { margin: 0; padding-left: 16px; font-size: var(--text-sm); }
  .note { font-size: var(--text-xs); margin: 10px 0 0; }
  details.indirect { margin-top: 10px; }
  details.indirect summary { cursor: pointer; font-size: var(--text-xs); }
  pre { margin: 0; white-space: pre-wrap; word-break: break-word; }

  /* specs, code, setup */
  .crumb { margin: 0 0 8px; }
  table.code { display: block; max-width: 100%; overflow-x: auto; font-family: var(--mono); font-size: 13.5px; background: var(--card); border: 1px solid var(--line); border-radius: var(--radius); }
  table.code td { border: none; padding: 0 10px; white-space: pre; }
  td.ln { text-align: right; width: 1%; user-select: none; }
  td.ln a { color: var(--muted); }
  tr.hl { background: color-mix(in srgb, var(--warn) 22%, transparent); }
  .rv { margin: 0 6px 0 0; vertical-align: middle; }
  span.rv { display: inline-block; width: 13px; text-align: center; }
  td.src { overflow-x: auto; }
  table.code.wrap td.src { white-space: pre-wrap; overflow-wrap: anywhere; }
  .crumb { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .crumb .small { font-size: var(--text-xs); padding: 2px 8px; }
  a.btn { background: var(--raised); color: var(--fg); border-radius: var(--radius); text-decoration: none; }
  a.btn:hover { background: var(--tint-btn); color: var(--accent); }
  /* highlight.js token colours (the markup comes from {@html}, hence :global) */
  :global(.hljs-comment, .hljs-quote) { color: var(--muted); font-style: italic; }
  :global(.hljs-keyword, .hljs-selector-tag, .hljs-literal, .hljs-built_in, .hljs-doctag) { color: light-dark(#8a2be2, #c9a7ff); }
  :global(.hljs-string, .hljs-regexp, .hljs-addition) { color: var(--ok); }
  :global(.hljs-number, .hljs-symbol, .hljs-bullet, .hljs-meta) { color: var(--warn); }
  :global(.hljs-title, .hljs-section, .hljs-attr, .hljs-attribute) { color: light-dark(#0b5cad, #7cb7ff); }
  :global(.hljs-type, .hljs-class .hljs-title) { color: light-dark(#0b7a75, #5eead4); }
  :global(.hljs-deletion) { color: var(--bad); }
  .block { margin-bottom: 10px; }
  .setup h3 { margin: 0 0 6px; font-size: 15px; }
  .setup p { margin: 6px 0; }
  .setup-actions { display: flex; flex-wrap: wrap; align-items: center; gap: var(--gap); }
  .setup-value { overflow-wrap: anywhere; }
  .setup p.setup-config { margin-top: var(--gap); padding-top: var(--gap); border-top: 1px solid var(--line); }
  .log { margin-top: 12px; padding: 10px 14px; background: var(--raised); border-radius: var(--radius); font-family: var(--mono); font-size: 12.5px; }
</style>
