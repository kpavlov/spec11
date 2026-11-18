// (WEB-35) Reviewed flags: {requirement ID: the commit it had when it was reviewed}, one record per repo in IndexedDB.
// Where storage is blocked the flags still work, for this session.
export const reviewed = $state<Record<string, string>>({});
let repo = "";
const run = <T>(mode: IDBTransactionMode, f: (s: IDBObjectStore) => IDBRequest<T>) =>
  new Promise<T>((ok, no) => {
    const q = indexedDB.open("spec11", 1);
    q.onupgradeneeded = () => q.result.createObjectStore("reviewed");
    q.onerror = () => no(q.error);
    q.onsuccess = () => {
      const r = f(q.result.transaction("reviewed", mode).objectStore("reviewed"));
      r.onsuccess = () => ok(r.result);
      r.onerror = () => no(r.error);
    };
  });

export async function loadReviewed(root: string) {
  repo = root;
  try { Object.assign(reviewed, (await run<Record<string, string> | undefined>("readonly", (s) => s.get(repo))) ?? {}); } catch { /* storage blocked */ }
}

/** Mark `id` reviewed at `commit`, or clear it. */
export function setReviewed(id: string, commit?: string) {
  if (commit) reviewed[id] = commit; else delete reviewed[id];
  run("readwrite", (s) => s.put($state.snapshot(reviewed), repo)).catch(() => {});
}
