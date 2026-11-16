// The URL is the state: every view and selection round-trips through parse/format, so back/forward, reload and shared links work.
//   /  (the dashboard; /report still works)   /requirements[/DOC-25][?state=none&q=text]   /modules[?module=core&q=text]   /tests[?module=core&state=untagged&q=text]
//   /specs[?file=04-documents.md&id=DOC-25]            /code?path=RunSolo/RunSolo/A.swift&line=12
//   /overlap[?id=DOC-25]   /uncovered[?id=…]   /setup
// File names go in the query, not the path, so a dev server never mistakes `x.md` or `A.swift` for a static file.
export const TABS = ["dashboard", "requirements", "modules", "tests", "specs", "overlap", "uncovered", "setup"] as const;
export type Tab = (typeof TABS)[number] | "code";

export type Route = { tab: Tab; id?: string; file?: string; path?: string; line?: number; state?: string; q?: string; module?: string };

export function parse(url: URL): Route {
  const [first, second] = url.pathname.split("/").filter(Boolean).map(decodeURIComponent);
  const tab = ([...TABS, "code"] as string[]).includes(first) ? (first as Tab) : "dashboard";
  const p = url.searchParams;
  const line = Number(p.get("line"));
  return {
    tab,
    id: (tab === "requirements" ? second : undefined) ?? p.get("id") ?? undefined,
    file: p.get("file") ?? undefined,
    path: p.get("path") ?? undefined,
    line: line > 0 ? line : undefined,
    state: p.get("state") ?? undefined,
    q: p.get("q") ?? undefined,
    module: p.get("module") ?? undefined,
  };
}

export function format(r: Route): string {
  const q = new URLSearchParams();
  const put = (k: string, v?: string | number) => v !== undefined && v !== "" && q.set(k, String(v));
  if (r.tab !== "requirements") put("id", r.id);
  put("file", r.file);
  put("path", r.path);
  put("line", r.line);
  if (r.state && r.state !== "all") put("state", r.state);
  put("q", r.q);
  put("module", r.module);
  const path = (r.tab === "dashboard" ? "/" : `/${r.tab}`) + (r.tab === "requirements" && r.id ? `/${encodeURIComponent(r.id)}` : "");
  const s = q.toString();
  return s ? `${path}?${s}` : path;
}
