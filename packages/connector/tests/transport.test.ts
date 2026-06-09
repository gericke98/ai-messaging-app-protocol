import { describe, it, expect, afterEach } from "vitest";
import { WebSocketServer } from "ws";
import { WebSocketTransport } from "../src/transport/websocket.js";

let server: WebSocketServer | null = null;
afterEach(() => { server?.close(); server = null; });

async function freePort(): Promise<number> {
  const { createServer } = await import("node:net");
  return new Promise((resolve) => {
    const s = createServer();
    s.listen(0, () => {
      const port = (s.address() as { port: number }).port;
      s.close(() => resolve(port));
    });
  });
}

describe("WebSocketTransport reconnect", () => {
  it("retries connect with backoff until the server comes up", async () => {
    const port = await freePort();
    const t = new WebSocketTransport({ retries: 4, backoffMs: 25 });
    // Server appears only after the first attempt has already failed.
    setTimeout(() => { server = new WebSocketServer({ port }); }, 40);
    await t.connect(`ws://127.0.0.1:${port}`);
    expect(t.isConnected()).toBe(true);
    t.close();
  });

  it("surfaces the error after exhausting retries", async () => {
    const port = await freePort(); // nothing listens here
    const t = new WebSocketTransport({ retries: 2, backoffMs: 5 });
    await expect(t.connect(`ws://127.0.0.1:${port}`)).rejects.toThrow();
  });

  it("reconnect() re-dials the last url", async () => {
    const port = await freePort();
    server = new WebSocketServer({ port });
    const t = new WebSocketTransport({ retries: 2, backoffMs: 5 });
    await t.connect(`ws://127.0.0.1:${port}`);
    expect(t.isConnected()).toBe(true);

    t.close();
    expect(t.isConnected()).toBe(false);

    await t.reconnect();
    expect(t.isConnected()).toBe(true);
    t.close();
  });
});
