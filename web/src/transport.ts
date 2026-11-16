import { Client, StreamableHTTPClientTransport } from "@modelcontextprotocol/client";

// `call(tool, args)` and `read(uri)` for both hosts: plain fetch to the Rust server, or the MCP Apps bridge when this page runs inside a host iframe.
export type Result<T = any> = { text: string; data: T };

export const embedded = window.parent !== window;

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
  // standalone: the same `tools/call` over /mcp (indexing can outlast the default 60 s timeout)
  const r = await (await connect()).callTool({ name: tool, arguments: args }, { timeout: tool === "spec_codegraph" ? 600_000 : undefined });
  const text = (r.content ?? []).map((c) => ("text" in c ? c.text : "")).join("\n");
  if (r.isError) throw new Error(text);
  return { text, data: r.structuredContent as T };
}

// Standalone: the official MCP client, pinned to 2026-07-28 (no `initialize`, no session; it sends `server/discover` once and then the per-request `_meta` headers).
let mcp: Promise<Client> | undefined;
const connect = () =>
  (mcp ??= (async () => {
    const c = new Client({ name: "spec11-web", version: "0.1.0" }, { versionNegotiation: { mode: { pin: "2026-07-28" } } });
    await c.connect(new StreamableHTTPClientTransport(new URL("/mcp", location.href)));
    return c;
  })().catch((e) => ((mcp = undefined), Promise.reject(e))));

// Push: `subscriptions/listen` on /mcp for these resources, `onChange` on each `notifications/resources/updated`.
// Standalone only: the host bridge is request/response and so it gets no updates (a no-op).
// A dropped stream is re-opened after a pause; the returned function stops it. `onState(true)` once the stream is open, `onState(false)` when it drops.
export function watch(uris: string[], onChange: () => void, onState: (up: boolean) => void = () => {}): () => void {
  if (embedded) return () => {};
  let stopped = false;
  let sub: { close(): Promise<void> } | undefined;
  (async function open() {
    while (!stopped) {
      try {
        const c = await connect();
        c.setNotificationHandler("notifications/resources/updated", () => onChange());
        sub = await c.listen({ resourceSubscriptions: uris });
        if (stopped) return void sub.close();
        onState(true);
        await (sub as any).closed;
        onState(false);
      } catch {
        onState(false);
        mcp = undefined; // reconnect from scratch (the server may have restarted)
      }
      await new Promise((r) => setTimeout(r, 3000));
    }
  })();
  return () => ((stopped = true), void sub?.close());
}

// An MCP resource (JSON): `resources/read` through the host bridge, or straight to `/mcp` with the SDK client.
export async function read<T = any>(uri: string): Promise<T> {
  let text: unknown;
  if (embedded) {
    await initBridge();
    text = (await rpc("resources/read", { uri })).contents?.[0]?.text;
  } else {
    text = ((await (await connect()).readResource({ uri })).contents[0] as { text?: string } | undefined)?.text;
  }
  return JSON.parse(String(text ?? "null"));
}
