// One vocabulary for "how it stands": a tone is set with data-tone="…" on any element (app.css turns it into --tone), so pills, dots, badges,
// row edges and sections all colour the same way.
export type Tone = "info" | "plum" | "blue" | "celery" | "cyan" | "teal" | "orange" | "red" | "indigo" | "brown" | "ok" | "warn" | "bad" | "accent" | "muted";

export const requirementLevelTone: Record<string, Tone> = { MUST: "plum", SHALL: "plum", SHOULD: "blue", MAY: "celery" };

// what the scanner saw per requirement state
export const stateTone: Record<string, Tone> = { tested: "cyan", "src-only": "orange", none: "red", done: "celery", manual: "teal", later: "indigo", ref: "brown" };
// the row edge shows only the states that carry a verdict
export const edgeTone = (state: string): Tone | undefined => (["orange", "red"].includes(stateTone[state]) ? stateTone[state] : undefined);
// setup and doctor statuses
export const levelTone = (level: string): Tone => (level === "ok" ? "ok" : level === "warn" ? "warn" : "bad");
// what happened to a file: read like a diff (added green, deleted red); the requirement pages keep red and amber for the requirement state
export const changeTone = (change: string): Tone => (change === "added" ? "ok" : change === "deleted" ? "bad" : "accent");
