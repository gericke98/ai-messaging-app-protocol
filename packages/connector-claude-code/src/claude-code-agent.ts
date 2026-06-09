import { AINMAgent } from "@ainm/connector";
import type { IncomingMessage, MessageContext } from "@ainm/connector";
import { spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";
import { checkClaudeAvailable } from "./check-cli.js";
import { buildClaudeArgs } from "./build-args.js";
import {
  parseClaudeEvent,
  extractTextContent,
  extractToolUses,
  type ClaudeAssistantEvent,
  type ClaudeResultEvent,
  type ClaudeSystemEvent,
} from "./parse-event.js";

export interface ClaudeCodeConnectorConfig {
  serverUrl: string;
  identity: string;
  name: string;
  workingDir?: string;
  claudeBinary?: string;
  sessionId?: string;
  permissionMode?: string;  // default: "auto"
  maxBudgetUsd?: number;
  bare?: boolean;
}

export async function startClaudeCodeConnector(
  config: ClaudeCodeConnectorConfig
): Promise<{ stop: () => void }> {
  const claudeBin = config.claudeBinary ?? "claude";
  let currentSessionId = config.sessionId;

  // Verify that the claude CLI is available before connecting
  const version = await checkClaudeAvailable(claudeBin);
  console.log(`Claude Code CLI version: ${version}`);

  const agent = new AINMAgent({
    server: config.serverUrl,
    identity: config.identity,
    name: config.name,
    capabilities: ["text", "code", "tool"],
  });

  // Track running child process for graceful shutdown
  let currentProc: ChildProcess | null = null;

  agent.onMessage(
    async (
      message: IncomingMessage,
      ctx: MessageContext
    ): Promise<string | void> => {
      // Runtime A2A: only respond when this agent was actually addressed.
      if (!message.addressed) {
        return;
      }
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

        const args = buildClaudeArgs({
          prompt,
          sessionId: currentSessionId,
          permissionMode: config.permissionMode,
          maxBudgetUsd: config.maxBudgetUsd,
          bare: config.bare,
        });

        const stream = ctx.stream();
        let fullText = "";
        // Track how much text we have already sent so we can compute deltas.
        // With --include-partial-messages each assistant event carries the FULL
        // accumulated text so far, not just the new tokens.
        let lastTextLength = 0;

        await new Promise<void>((resolve, reject) => {
          const proc = spawn(claudeBin, args, {
            cwd: config.workingDir ?? process.cwd(),
            stdio: ["pipe", "pipe", "pipe"],
            env: { ...process.env },
          });

          currentProc = proc;

          const rl = createInterface({ input: proc.stdout });

          rl.on("line", (line: string) => {
            const event = parseClaudeEvent(line);
            if (event === null) return;

            if (event.type === "system") {
              const sysEv = event as ClaudeSystemEvent;
              currentSessionId = sysEv.session_id;
              console.log(
                `Claude session started — id: ${sysEv.session_id}, model: ${sysEv.model}, tools: ${sysEv.tools.join(", ")}`
              );
              return;
            }

            if (event.type === "assistant") {
              const assistantEv = event as ClaudeAssistantEvent;

              // Stream text deltas (handling accumulated text from --include-partial-messages)
              const currentText = extractTextContent(assistantEv);
              if (currentText.length > lastTextLength) {
                const delta = currentText.slice(lastTextLength);
                stream.delta(delta);
                fullText = currentText;
                lastTextLength = currentText.length;
              }

              // Emit tool started events for each tool_use block
              const toolUses = extractToolUses(assistantEv);
              for (const tool of toolUses) {
                ctx.emitToolStarted(tool.name, tool.input);
              }

              return;
            }

            if (event.type === "result") {
              const resultEv = event as ClaudeResultEvent;
              currentSessionId = resultEv.session_id;

              if (resultEv.is_error) {
                // Error result — end stream and reject with the error message
                stream.end();
                reject(new Error(resultEv.result));
                return;
              }

              // Successful result — end the stream
              stream.end();
              resolve();
              return;
            }
          });

          let stderr = "";
          proc.stderr.on("data", (data: Buffer) => {
            stderr += data.toString();
          });

          proc.on("close", (code: number | null) => {
            currentProc = null;
            // If the promise was already settled by a result event, this is a no-op.
            // Otherwise treat a non-zero exit as an error.
            if (code !== 0 && !fullText) {
              stream.end();
              reject(
                new Error(
                  `Claude Code exited with code ${code}: ${stderr.slice(0, 500)}`
                )
              );
            } else {
              // Ensure stream ends if process closes without a result event
              stream.end();
              resolve();
            }
          });

          proc.on("error", (err: Error) => {
            currentProc = null;
            stream.end();
            reject(err);
          });
        });
      } catch (error) {
        return `Error: ${error instanceof Error ? error.message : String(error)}`;
      }
    }
  );

  console.log(`Connecting Claude Code connector to ${config.serverUrl}...`);
  if (config.workingDir) {
    console.log(`Working directory: ${config.workingDir}`);
  }
  if (currentSessionId) {
    console.log(`Resuming session: ${currentSessionId}`);
  }

  await agent.connect();
  console.log(`Claude Code connector ready. Identity: ${config.identity}`);

  // Graceful shutdown: kill any running child process and disconnect the agent
  const stop = (): void => {
    if (currentProc) {
      currentProc.kill();
      currentProc = null;
    }
    agent.disconnect();
  };

  // Register OS signal handlers for clean shutdown
  const handleSignal = (): void => {
    stop();
    process.exit(0);
  };
  process.once("SIGTERM", handleSignal);
  process.once("SIGINT", handleSignal);

  return { stop };
}
