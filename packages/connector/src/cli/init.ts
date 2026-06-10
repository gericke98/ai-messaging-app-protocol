/**
 * `ainm-connector init` core (M5 instant-token fast path).
 *
 * Fresh machine -> working session in one command: generate an Ed25519
 * keypair, instant-register it (POST /agents/instant-register), persist
 * credentials, connect with the signed handshake, send a test message.
 * Re-running with the same credentials file (or --private-key backing it)
 * reuses the identity without re-registration.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { randomUUID } from "node:crypto";

import {
  base64urlDecode,
  base64urlEncode,
  generateKeypair,
  keypairFromPrivateKey,
  type Keypair,
} from "../identity/keypair.js";
import { signEnvelope } from "../identity/signing.js";
import { performHandshake } from "../protocol/handshake.js";
import { buildNotification } from "../protocol/rpc.js";
import { WebSocketTransport } from "../transport/websocket.js";

export interface Credentials {
  server: string;
  address: string;
  /** base64url-encoded Ed25519 private key. */
  privateKey: string;
}

export interface InitOptions {
  server: string;
  name: string;
  credentials: string;
  /** base64url private key for a fresh registration with a static identity. */
  privateKey?: string;
}

export interface InitDeps {
  fetchFn?: typeof fetch;
  connectAndTest?: (wsUrl: string, creds: Credentials) => Promise<void>;
  log?: (msg: string) => void;
}

export interface InitResult {
  address: string;
  wsUrl: string;
  reused: boolean;
}

/** ws(s):// -> http(s)://, host[:port] only (registration is served at the root). */
export function httpBaseFromWs(server: string): string {
  const u = new URL(server);
  const proto = u.protocol === "wss:" ? "https:" : u.protocol === "ws:" ? "http:" : u.protocol;
  return `${proto}//${u.host}`;
}

/** Normalize any server URL to the ws(s)://host[:port]/ws connect endpoint. */
export function wsUrlFrom(server: string): string {
  const u = new URL(server);
  const proto = u.protocol === "https:" ? "wss:" : u.protocol === "http:" ? "ws:" : u.protocol;
  const path = u.pathname === "" || u.pathname === "/" ? "/ws" : u.pathname;
  return `${proto}//${u.host}${path}`;
}

export function loadCredentials(path: string): Credentials | null {
  if (!existsSync(path)) return null;
  return JSON.parse(readFileSync(path, "utf8")) as Credentials;
}

export function saveCredentials(path: string, creds: Credentials): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, JSON.stringify(creds, null, 2) + "\n", { mode: 0o600 });
}

export async function runInit(opts: InitOptions, deps: InitDeps = {}): Promise<InitResult> {
  const fetchFn = deps.fetchFn ?? fetch;
  const log = deps.log ?? (() => {});
  const connectAndTest = deps.connectAndTest ?? defaultConnectAndTest;
  const wsUrl = wsUrlFrom(opts.server);

  // Reuse a persisted identity: rejoin without re-registration.
  const existing = loadCredentials(opts.credentials);
  if (existing) {
    log(`reusing identity ${existing.address} from ${opts.credentials}`);
    await connectAndTest(wsUrl, existing);
    return { address: existing.address, wsUrl, reused: true };
  }

  const keypair: Keypair = opts.privateKey
    ? await keypairFromPrivateKey(base64urlDecode(opts.privateKey))
    : await generateKeypair();

  log("registering with the server");
  const resp = await fetchFn(`${httpBaseFromWs(opts.server)}/agents/instant-register`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      name: opts.name,
      public_key: base64urlEncode(keypair.publicKey),
    }),
  });
  if (!resp.ok) {
    const detail = await resp.text();
    throw new Error(`instant registration failed (${resp.status}): ${detail}`);
  }
  const { address } = (await resp.json()) as { address: string };

  const creds: Credentials = {
    server: wsUrl,
    address,
    privateKey: base64urlEncode(keypair.privateKey),
  };
  saveCredentials(opts.credentials, creds);
  log(`credentials saved to ${opts.credentials}`);

  await connectAndTest(wsUrl, creds);
  return { address, wsUrl, reused: false };
}

/** Live path: signed handshake + a best-effort test message, then close. */
async function defaultConnectAndTest(wsUrl: string, creds: Credentials): Promise<void> {
  const keypair = await keypairFromPrivateKey(base64urlDecode(creds.privateKey));
  const transport = new WebSocketTransport();
  await transport.connect(wsUrl);
  try {
    await performHandshake(transport, {
      identity: creds.address,
      name: creds.address.split("@")[0],
      capabilities: ["text"],
      keypair,
    });

    const envelope = await signEnvelope(keypair, {
      id: randomUUID(),
      from: creds.address,
      thread_id: `init-${randomUUID()}`,
      timestamp: new Date().toISOString(),
      type: "message",
      content: [
        {
          type: "text",
          version: "1.0",
          source: "agent",
          text: "instant-token init test message",
          fallback: "instant-token init test message",
        },
      ],
    });
    const note = buildNotification("ainm.message.send", envelope as Record<string, unknown>);
    await transport.send(JSON.stringify(note));
  } finally {
    transport.close();
  }
}
