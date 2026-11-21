import { mount, unmount } from "svelte";
import type { Attachment } from "svelte/attachments";
import LevelPill from "./LevelPill.svelte";

// markdown.ts marks RFC 2119 words as <span class="rfc" data-level="MUST">; {@html} cannot hold components, so this swaps each one for the LevelPill.
// Usage: <div {@attach levelPills(html)}>{@html html}</div> (passing the html re-runs it when the text changes).
export const levelPills = (_html: string): Attachment => (root) => {
  const pills = [...root.querySelectorAll<HTMLElement>("span.rfc[data-level]")].map((el) => {
    el.textContent = "";
    return mount(LevelPill, { target: el, props: { level: el.dataset.level! } });
  });
  return () => pills.forEach((p) => void unmount(p));
};
