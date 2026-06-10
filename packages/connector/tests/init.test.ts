import { describe, it, expect } from "vitest";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  httpBaseFromWs,
  wsUrlFrom,
  loadCredentials,
  saveCredentials,
  runInit,
} from "../src/cli/init.js";
import { generateKeypair, base64urlEncode } from "../src/identity/keypair.js";

describe("url derivation", () => {
  it("maps ws schemes to http schemes and strips the path", () => {
    expect(httpBaseFromWs("wss://ainm.example.com/ws")).toBe("https://ainm.example.com");
    expect(httpBaseFromWs("ws://localhost:8080/ws")).toBe("http://localhost:8080");
    expect(httpBaseFromWs("ws://localhost:8080")).toBe("http://localhost:8080");
  });

  it("normalizes the ws url to end in /ws", () => {
    expect(wsUrlFrom("wss://ainm.example.com")).toBe("wss://ainm.example.com/ws");
    expect(wsUrlFrom("ws://localhost:8080/ws")).toBe("ws://localhost:8080/ws");
  });
});

describe("credentials persistence", () => {
  it("round-trips and returns null when absent", () => {
    const dir = mkdtempSync(join(tmpdir(), "ainm-init-"));
    const path = join(dir, "creds.json");
    expect(loadCredentials(path)).toBeNull();

    const creds = {
      server: "wss://x/ws",
      address: "bot@x",
      privateKey: "QUJD",
    };
    saveCredentials(path, creds);
    expect(loadCredentials(path)).toEqual(creds);
  });
});

describe("runInit", () => {
  function deps(overrides: Partial<Parameters<typeof runInit>[1]> = {}) {
    const calls: { registered: unknown[]; connected: string[] } = {
      registered: [],
      connected: [],
    };
    return {
      calls,
      deps: {
        fetchFn: (async (url: string, init?: { body?: string }) => {
          calls.registered.push({ url, body: JSON.parse(init?.body ?? "{}") });
          return {
            ok: true,
            status: 201,
            json: async () => ({ address: "minted-bot@srv.local" }),
            text: async () => "",
          };
        }) as unknown as typeof fetch,
        connectAndTest: async (wsUrl: string) => {
          calls.connected.push(wsUrl);
        },
        log: () => {},
        ...overrides,
      },
    };
  }

  it("fresh init registers, persists credentials, and connects", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ainm-init-"));
    const credsPath = join(dir, "creds.json");
    const { calls, deps: d } = deps();

    const result = await runInit(
      { server: "wss://srv.local", name: "My Bot", credentials: credsPath },
      d,
    );

    expect(result.address).toBe("minted-bot@srv.local");
    expect(result.reused).toBe(false);
    expect(calls.registered).toHaveLength(1);
    const reg = calls.registered[0] as { url: string; body: { name: string; public_key: string } };
    expect(reg.url).toBe("https://srv.local/agents/instant-register");
    expect(reg.body.name).toBe("My Bot");
    expect(reg.body.public_key.length).toBeGreaterThan(20);
    expect(calls.connected).toEqual(["wss://srv.local/ws"]);

    const saved = loadCredentials(credsPath);
    expect(saved?.address).toBe("minted-bot@srv.local");
    expect(saved?.privateKey.length).toBeGreaterThan(20);
  });

  it("existing credentials skip registration entirely", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ainm-init-"));
    const credsPath = join(dir, "creds.json");
    const kp = await generateKeypair();
    saveCredentials(credsPath, {
      server: "wss://srv.local/ws",
      address: "kept-bot@srv.local",
      privateKey: base64urlEncode(kp.privateKey),
    });
    const { calls, deps: d } = deps();

    const result = await runInit(
      { server: "wss://srv.local", name: "ignored", credentials: credsPath },
      d,
    );

    expect(result.address).toBe("kept-bot@srv.local");
    expect(result.reused).toBe(true);
    expect(calls.registered).toHaveLength(0);
    expect(calls.connected).toEqual(["wss://srv.local/ws"]);
  });

  it("surfaces a registration failure with the server's message", async () => {
    const dir = mkdtempSync(join(tmpdir(), "ainm-init-"));
    const { deps: d } = deps();
    d.fetchFn = (async () => ({
      ok: false,
      status: 403,
      json: async () => ({}),
      text: async () => "instant registration is disabled on this server",
    })) as unknown as typeof fetch;

    await expect(
      runInit(
        { server: "wss://srv.local", name: "x", credentials: join(dir, "c.json") },
        d,
      ),
    ).rejects.toThrow(/disabled/);
  });
});
