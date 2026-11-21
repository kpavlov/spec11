// (WEB-35) Reviewed flags: {requirement ID: the commit it had when it was reviewed}, one record per repo in IndexedDB.
// Where storage is blocked the flags still work, for this session.
import { db, run } from "$lib/db.svelte";

export const reviewed = $state<Record<string, string>>({});

export async function loadReviewed(root: string) {
  try { Object.assign(reviewed, (await run<Record<string, string> | undefined>("reviewed", "readonly", (s) => s.get(root))) ?? {}); } catch { /* storage blocked */ }
  finally { db.root = root; }
}

/** Mark `id` reviewed at `commit`, or clear it. */
export function setReviewed(id: string, commit?: string) {
  if (commit) reviewed[id] = commit; else delete reviewed[id];
  const root = db.root;
  if (root !== null) run("reviewed", "readwrite", (s) => s.put($state.snapshot(reviewed), root)).catch(() => {});
}
