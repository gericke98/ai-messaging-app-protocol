import { describe, it, expect } from "vitest";
import { generateKeypair, exportPublicKey } from "../src/identity/keypair.js";
import { signInit, signEnvelope, verifyEnvelope } from "../src/identity/signing.js";

describe("Ed25519 identity", () => {
  it("generates a keypair", async () => {
    const kp = await generateKeypair();
    expect(kp.privateKey).toBeInstanceOf(Uint8Array);
    expect(kp.publicKey).toBeInstanceOf(Uint8Array);
    expect(kp.privateKey.length).toBe(32);
    expect(kp.publicKey.length).toBe(32);
  });

  it("exports public key as base64url", async () => {
    const kp = await generateKeypair();
    const exported = exportPublicKey(kp);
    expect(typeof exported).toBe("string");
    expect(exported.length).toBeGreaterThan(0);
    expect(exported).not.toContain("=");
    expect(exported).not.toContain("+");
    expect(exported).not.toContain("/");
  });
});

describe("message signing", () => {
  it("signs an init payload and returns the bound timestamp", async () => {
    const kp = await generateKeypair();
    const { signature, timestamp } = await signInit(kp, "agent@test.local");
    expect(typeof signature).toBe("string");
    expect(signature.length).toBeGreaterThan(0);
    expect(Number.isNaN(new Date(timestamp).getTime())).toBe(false);
  });

  it("signs and verifies an envelope", async () => {
    const kp = await generateKeypair();
    const envelope = {
      id: "msg-1",
      from: "agent@test.local",
      thread_id: "thread-1",
      timestamp: "2026-01-01T00:00:00.000Z",
      type: "message",
      content: [{ type: "text", text: "hello" }],
    };
    const signed = await signEnvelope(kp, envelope);
    expect(signed.signature).toBeDefined();
    expect(typeof signed.signature).toBe("string");

    const valid = await verifyEnvelope(kp.publicKey, signed);
    expect(valid).toBe(true);
  });

  it("rejects tampered envelope", async () => {
    const kp = await generateKeypair();
    const envelope = {
      id: "msg-2",
      from: "agent@test.local",
      thread_id: "thread-1",
      timestamp: "2026-01-01T00:00:00.000Z",
      type: "message",
      content: [{ type: "text", text: "hello" }],
    };
    const signed = await signEnvelope(kp, envelope);
    (signed as any).content = [{ type: "text", text: "tampered" }];

    const valid = await verifyEnvelope(kp.publicKey, signed);
    expect(valid).toBe(false);
  });
});
