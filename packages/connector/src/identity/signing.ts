import * as ed from "@noble/ed25519";
import canonicalize from "canonicalize";
import { base64urlEncode, base64urlDecode, type Keypair } from "./keypair.js";

export async function signInit(keypair: Keypair, identity: string): Promise<string> {
  const timestamp = new Date().toISOString();
  const payload = `${identity}|${timestamp}`;
  const sig = await ed.signAsync(new TextEncoder().encode(payload), keypair.privateKey);
  return base64urlEncode(new Uint8Array(sig));
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
