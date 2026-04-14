import { AINMAgent } from "@ainm/connector";
import type { IncomingMessage, MessageContext } from "@ainm/connector";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

export interface ClaudeCodeConnectorConfig {
  serverUrl: string;
  identity: string;
  name: string;
  workingDir?: string;
  claudeBinary?: string;
  sessionId?: string;
}

/**
 * Parsed event from Claude Code's NDJSON stream output.
 */
interface ClaudeCodeEvent {
  type: string;
  subtype?: string;
  content?: string;
  result?: string;
  session_id?: string;
  tool_name?: string;
  input?: Record<string, unknown>;
}

export async function startClaudeCodeConnector(
  config: ClaudeCodeConnectorConfig
): Promise<void> {
  const claudeBin = config.claudeBinary ?? "claude";
  let currentSessionId = config.sessionId;

  const agent = new AINMAgent({
    server: config.serverUrl,
    identity: config.identity,
    name: config.name,
    capabilities: ["text", "code", "tool"],
  });

  agent.onMessage(
    async (
      message: IncomingMessage,
      ctx: MessageContext
    ): Promise<string | void> => {
      try {
        // Pull compound memory briefing for context
        let briefingContext = "";
        try {
          const briefing = await ctx.memory.getBriefing();
          if (briefing && typeof briefing === "object") {
            briefingContext = `\n\nUser context from AINM memory graph:\n${JSON.stringify(briefing, null, 2)}\n\n`;
          }
        } catch {
          // Briefing is optional -- continue without it
        }

        const prompt = briefingContext + message.text;

        // Build claude CLI args
        const args = ["-p", prompt, "--output-format", "stream-json"];
        if (currentSessionId) {
          args.push("--resume", currentSessionId);
        }

        const stream = ctx.stream();
        let fullText = "";

        await new Promise<void>((resolve, reject) => {
          const proc = spawn(claudeBin, args, {
            cwd: config.workingDir ?? process.cwd(),
            stdio: ["pipe", "pipe", "pipe"],
            env: { ...process.env },
          });

          const rl = createInterface({ input: proc.stdout });

          rl.on("line", (line: string) => {
            try {
              const event = JSON.parse(line) as ClaudeCodeEvent;

              // Stream text deltas from assistant responses
              if (event.type === "assistant" && event.subtype === "text") {
                const content = event.content ?? "";
                stream.delta(content);
                fullText += content;
              }

              // Capture session ID for conversation continuity
              if (event.type === "result" && event.session_id) {
                currentSessionId = event.session_id;
              }

              // Forward tool use events so the iOS app can show what Claude Code is doing
              if (event.type === "assistant" && event.subtype === "tool_use") {
                ctx.emitToolStarted(
                  event.tool_name ?? "unknown",
                  event.input ?? {}
                );
              }

              if (
                event.type === "assistant" &&
                event.subtype === "tool_result"
              ) {
                ctx.emitToolCompleted(
                  event.tool_name ?? "unknown",
                  event.content ?? ""
                );
              }
            } catch {
              // Skip non-JSON lines (e.g. stderr bleed, blank lines)
            }
          });

          let stderr = "";
          proc.stderr.on("data", (data: Buffer) => {
            stderr += data.toString();
          });

          proc.on("close", (code: number | null) => {
            stream.end();
            if (code !== 0 && !fullText) {
              reject(
                new Error(
                  `Claude Code exited with code ${code}: ${stderr.slice(0, 500)}`
                )
              );
            } else {
              resolve();
            }
          });

          proc.on("error", (err: Error) => {
            stream.end();
            reject(err);
          });
        });
      } catch (error) {
        return `Error: ${error instanceof Error ? error.message : String(error)}`;
      }
    }
  );

  console.log(
    `Connecting Claude Code connector to ${config.serverUrl}...`
  );
  if (config.workingDir) {
    console.log(`Working directory: ${config.workingDir}`);
  }
  if (currentSessionId) {
    console.log(`Resuming session: ${currentSessionId}`);
  }
  await agent.connect();
  console.log(`Claude Code connector ready. Identity: ${config.identity}`);

  // Keep process alive until terminated
  await new Promise<void>(() => {});
}
