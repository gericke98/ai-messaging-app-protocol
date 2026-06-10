import * as ed from "@noble/ed25519";
import canonicalize from "canonicalize";
import { base64urlEncode, base64urlDecode, type Keypair } from "./keypair.js";

/// Sign the init binding `${identity}|${timestamp}`. The timestamp MUST be sent
/// alongside the signature — the server's verify_init reconstructs the payload
/// from `params.timestamp` and rejects the handshake when it is absent or stale.
export async function signInit(
  keypair: Keypair,
  identity: string,
): Promise<{ signature: string; timestamp: string }> {
  const timestamp = new Date().toISOString();
  const payload = `${identity}|${timestamp}`;
  const sig = await ed.signAsync(new TextEncoder().encode(payload), keypair.privateKey);
  return { signature: base64urlEncode(new Uint8Array(sig)), timestamp };
}

export async function signEnvelope(
  keypair: Keypair,
  envelope: Record<string, unknown>
): Promise<Record<string, unknown>> {
  const copy = { ...envelope };
  delete copy.signature;

  const canonical = canonicalize(copy);
  if (!canonical) throw new Error("Canonicalization failed");

  const sig = await ed.signAsync(new TextEncoder().encode(canonical), keypair.privateKey);
  return { ...envelope, signature: base64urlEncode(new Uint8Array(sig)) };
}

export async function verifyEnvelope(
  publicKey: Uint8Array,
  envelope: Record<string, unknown>
): Promise<boolean> {
  const sig = envelope.signature;
  if (typeof sig !== "string") return false;

  const copy = { ...envelope };
  delete copy.signature;

  const canonical = canonicalize(copy);
  if (!canonical) return false;

  try {
    return await ed.verifyAsync(base64urlDecode(sig), new TextEncoder().encode(canonical), publicKey);
  } catch {
    return false;
  }
}
