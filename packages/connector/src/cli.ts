#!/usr/bin/env node
/**
 * ainm-connector — connector CLI.
 *
 *   npx @ainm/connector init --server wss://your-server [--name my-agent]
 *                            [--credentials ./.ainm-credentials.json]
 *                            [--private-key <base64url>]
 *
 * Fresh machine to working session in one command (M5 instant-token path).
 */
import { runInit } from "./cli/init.js";

function flag(args: string[], name: string): string | undefined {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
}

function usage(): never {
  console.log(
    [
      "usage: ainm-connector init --server <ws(s)://host[:port]> [options]",
      "",
      "options:",
      "  --name <name>           agent name to register (default: my-agent)",
      "  --credentials <path>    identity file (default: ./.ainm-credentials.json)",
      "  --private-key <b64url>  register with a static Ed25519 private key",
    ].join("\n"),
  );
  process.exit(2);
}

async function main(): Promise<void> {
  const [, , command, ...args] = process.argv;
  if (command !== "init") usage();
  const server = flag(args, "--server");
  if (!server) usage();

  const started = Date.now();
  const step = (msg: string) => console.log(`  ✓ ${msg}`);

  console.log("ainm-connector init");
  const result = await runInit(
    {
      server,
      name: flag(args, "--name") ?? "my-agent",
      credentials: flag(args, "--credentials") ?? "./.ainm-credentials.json",
      privateKey: flag(args, "--private-key"),
    },
    { log: step },
  );
  step("connected: signed handshake accepted");
  step("test message sent");

  const secs = ((Date.now() - started) / 1000).toFixed(1);
  console.log("");
  console.log(`${result.reused ? "rejoined" : "registered"} as ${result.address} in ${secs}s`);
  console.log(`server: ${result.wsUrl}`);
  console.log("");
  console.log("next: build your agent with @ainm/connector —");
  console.log(`  new AINMAgent({ server: "${result.wsUrl}", identity: "${result.address}", ... })`);
}

main().catch((err) => {
  console.error(`error: ${err instanceof Error ? err.message : err}`);
  process.exit(1);
});
