// One vocabulary for "how it stands": a tone is set with data-tone="…" on any element (app.css turns it into --tone), so pills, dots, badges,
// row edges and sections all colour the same way.
export type Tone = "ok" | "warn" | "bad" | "accent" | "muted";

// what the scanner saw per requirement state (evidence, not truth: `ok` means a test carries the tag)
export const stateTone: Record<string, Tone> = { tested: "ok", "src-only": "warn", none: "bad", done: "accent", manual: "muted", later: "muted", ref: "muted" };
// the row edge shows only the states that carry a verdict
export const edgeTone = (state: string): Tone | undefined => (["ok", "warn", "bad"].includes(stateTone[state]) ? stateTone[state] : undefined);
// setup and doctor statuses
export const levelTone = (level: string): Tone => (level === "ok" ? "ok" : level === "warn" ? "warn" : "bad");
