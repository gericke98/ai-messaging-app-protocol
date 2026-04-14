import { sha512 } from "@noble/hashes/sha2.js";
import * as ed from "@noble/ed25519";
ed.etc.sha512Sync = sha512;

export interface Keypair {
  privateKey: Uint8Array;
  publicKey: Uint8Array;
}

export async function generateKeypair(): Promise<Keypair> {
  const privateKey = ed.utils.randomPrivateKey();
  const publicKey = await ed.getPublicKeyAsync(privateKey);
  return { privateKey, publicKey };
}

export function exportPublicKey(keypair: Keypair): string {
  return base64urlEncode(keypair.publicKey);
}

export function base64urlEncode(bytes: Uint8Array): string {
  const base64 = Buffer.from(bytes).toString("base64");
  return base64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function base64urlDecode(str: string): Uint8Array {
  const base64 = str.replace(/-/g, "+").replace(/_/g, "/");
  return new Uint8Array(Buffer.from(base64, "base64"));
}
