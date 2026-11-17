// Split-pane layout shared by every page that lists on one side and previews on the other:
// the orientation is one choice for the whole dashboard, the size of the list pane is remembered per page `name` ("" = requirements and tests).
const P = "spec11.splitpos";
const clampPos = (n: number) => Math.min(85, Math.max(15, Math.round(n)));
const read = (k: string) => { try { return localStorage.getItem(k); } catch { return null; } };
const write = (k: string, v: string | null) => { try { v === null ? localStorage.removeItem(k) : localStorage.setItem(k, v); } catch { /* storage blocked */ } };

const loadPos = () => {
  const out: Record<string, number> = {};
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i)!;
      const n = Number(localStorage.getItem(k));
      if (k.startsWith(P) && n) out[k.slice(P.length)] = clampPos(n);
    }
  } catch { /* storage blocked */ }
  return out;
};

export const layout = $state<{ split: "v" | "h"; pos: Record<string, number> }>({ split: read("spec11.split") === "h" ? "h" : "v", pos: loadPos() });

export const toggleSplit = () => { layout.split = layout.split === "v" ? "h" : "v"; write("spec11.split", layout.split); };
// percent of the list pane: dragged, else `def` beside the preview and at most half on top of it
export const listPct = (name: string, def: number) => layout.pos[name] ?? (layout.split === "v" ? def : Math.min(50, def));
export const setPos = (name: string, n: number | null, keep = true) => {
  if (n === null) delete layout.pos[name]; else layout.pos[name] = clampPos(n);
  if (keep) write(P + name, n === null ? null : String(layout.pos[name]));
};
// beside (wide screens, orientation "v") or stacked
export const sideBySide = () => layout.split === "v" && matchMedia("(min-width: 1000px)").matches;

// What the preview of a changed item shows: the item as it is, or what changed against HEAD. One choice for every preview.
export const preview = $state<{ view: "actual" | "diff" }>({ view: read("spec11.view") === "diff" ? "diff" : "actual" });
// Whether long diff lines wrap or scroll sideways; remembered.
export const diffWrap = $state({ on: read("spec11.diffwrap") === "1" });
export const setDiffWrap = (on: boolean) => { diffWrap.on = on; write("spec11.diffwrap", on ? "1" : null); };
export const setView = (v: string) => { preview.view = v === "diff" ? "diff" : "actual"; write("spec11.view", preview.view); };
