import { AINMAgent } from "@ainm/connector";
import type { IncomingMessage, MessageContext } from "@ainm/connector";

export interface OpenAIConnectorConfig {
  serverUrl: string;
  identity: string;
  name: string;
  model: string;
  baseUrl: string;
  apiKey: string;
  systemPrompt?: string;
}

interface ChatCompletionChunkDelta {
  content?: string;
}

interface ChatCompletionChunkChoice {
  delta: ChatCompletionChunkDelta;
}

interface ChatCompletionChunk {
  choices?: ChatCompletionChunkChoice[];
}

export async function startOpenAIConnector(
  config: OpenAIConnectorConfig
): Promise<void> {
  const agent = new AINMAgent({
    server: config.serverUrl,
    identity: config.identity,
    name: config.name,
    capabilities: ["text", "code"],
  });

  agent.onMessage(
    async (message: IncomingMessage, ctx: MessageContext): Promise<string | void> => {
      try {
        // Pull compound memory briefing for context
        let briefingContext = "";
        try {
          const briefing = await ctx.memory.getBriefing();
          if (briefing && typeof briefing === "object") {
            briefingContext = `\n\nUser context from memory graph:\n${JSON.stringify(briefing, null, 2)}`;
          }
        } catch {
          // Briefing is optional -- continue without it
        }

        const systemMessage =
          (config.systemPrompt ?? "You are a helpful assistant.") +
          briefingContext;

        // Build headers -- only include Authorization if an API key is configured
        const headers: Record<string, string> = {
          "Content-Type": "application/json",
        };
        if (config.apiKey) {
          headers["Authorization"] = `Bearer ${config.apiKey}`;
        }

        // Call OpenAI-compatible chat completions API with streaming
        const response = await fetch(
          `${config.baseUrl}/chat/completions`,
          {
            method: "POST",
            headers,
            body: JSON.stringify({
              model: config.model,
              stream: true,
              messages: [
                { role: "system", content: systemMessage },
                { role: "user", content: message.text },
              ],
            }),
          }
        );

        if (!response.ok) {
          const errorText = await response.text();
          return `Error from LLM API: ${response.status} ${errorText}`;
        }

        // Parse SSE stream
        const stream = ctx.stream();
        const reader = response.body?.getReader();
        if (!reader) {
          return "No response body from LLM API";
        }

        const decoder = new TextDecoder();
        let buffer = "";

        try {
          while (true) {
            const { done, value } = await reader.read();
            if (done) break;

            buffer += decoder.decode(value, { stream: true });
            const lines = buffer.split("\n");
            // Keep the last partial line in the buffer
            buffer = lines.pop() ?? "";

            for (const line of lines) {
              const trimmed = line.trim();
              if (!trimmed.startsWith("data: ")) continue;

              const data = trimmed.slice(6);
              if (data === "[DONE]") continue;

              try {
                const parsed = JSON.parse(data) as ChatCompletionChunk;
                const content = parsed.choices?.[0]?.delta?.content;
                if (content) {
                  stream.delta(content);
                }
              } catch {
                // Skip malformed SSE data lines
              }
            }
          }

          // Process any remaining data in the buffer
          if (buffer.trim()) {
            const trimmed = buffer.trim();
            if (trimmed.startsWith("data: ")) {
              const data = trimmed.slice(6);
              if (data !== "[DONE]") {
                try {
                  const parsed = JSON.parse(data) as ChatCompletionChunk;
                  const content = parsed.choices?.[0]?.delta?.content;
                  if (content) {
                    stream.delta(content);
                  }
                } catch {
                  // Skip malformed final line
                }
              }
            }
          }
        } finally {
          stream.end();
        }
      } catch (error) {
        return `Error: ${error instanceof Error ? error.message : String(error)}`;
      }
    }
  );

  console.log(
    `Connecting ${config.name} (${config.model}) to ${config.serverUrl}...`
  );
  await agent.connect();
  console.log(`${config.name} connected and ready.`);

  // Keep process alive until terminated
  await new Promise<void>(() => {});
}
