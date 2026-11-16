// One `call(tool, args)` for both hosts: plain fetch to the Rust server, or the MCP Apps bridge when this page runs inside a host iframe.
export type Result<T = any> = { text: string; data: T };

const embedded = window.parent !== window;

let seq = 0;
const pending = new Map<number, { ok: (v: any) => void; fail: (e: Error) => void }>();

function rpc(method: string, params: unknown): Promise<any> {
  const id = ++seq;
  return new Promise((ok, fail) => {
    pending.set(id, { ok, fail });
    window.parent.postMessage({ jsonrpc: "2.0", id, method, params }, "*");
  });
}

let ready: Promise<unknown> | undefined;
function initBridge() {
  ready ??= (() => {
    window.addEventListener("message", (e) => {
      const m = e.data;
      if (e.source !== window.parent || !m || m.jsonrpc !== "2.0" || !pending.has(m.id)) return;
      const p = pending.get(m.id)!;
      pending.delete(m.id);
      m.error ? p.fail(new Error(m.error.message ?? "host error")) : p.ok(m.result);
    });
    return rpc("ui/initialize", { protocolVersion: "2025-11-21", appInfo: { name: "spec11", version: "0.1.0" }, appCapabilities: {} }).then(() =>
      window.parent.postMessage({ jsonrpc: "2.0", method: "ui/notifications/initialized", params: {} }, "*"),
    );
  })();
  return ready;
}

export async function call<T = any>(tool: string, args: Record<string, unknown> = {}): Promise<Result<T>> {
  if (embedded) {
    await initBridge();
    const r = await rpc("tools/call", { name: tool, arguments: args });
    const text = (r.content ?? []).map((c: { text?: string }) => c.text ?? "").join("\n");
    if (r.isError) throw new Error(text);
    return { text, data: r.structuredContent as T };
  }
  const res = await fetch(`/api/call/${tool}`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(args) });
  const body = await res.json();
  if (!res.ok) throw new Error(body.error ?? res.statusText);
  return body;
}
