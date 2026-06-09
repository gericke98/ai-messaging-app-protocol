import { buildNotification } from "./rpc.js";
import { randomUUID } from "node:crypto";

export interface StreamHelper {
  start: () => Promise<void>;
  delta: (text: string) => Promise<void>;
  end: (fullText: string) => Promise<void>;
  streamId: string;
  messageId: string;
}

export function createStreamHelper(
  send: (data: string) => Promise<void>,
  fromIdentity: string,
  threadId: string
): StreamHelper {
  const streamId = randomUUID();
  const messageId = randomUUID();
  let accumulated = "";

  return {
    streamId,
    messageId,

    async start() {
      const notification = buildNotification("ainm.stream.start", {
        stream_id: streamId,
        message_id: messageId,
        from: fromIdentity,
        thread_id: threadId,
        timestamp: new Date().toISOString(),
        blocks: [{ type: "text", version: "1.0", source: "agent" }],
      });
      await send(JSON.stringify(notification));
    },

    async delta(text: string) {
      accumulated += text;
      const notification = buildNotification("ainm.stream.delta", {
        stream_id: streamId,
        block_index: 0,
        delta: text,
      });
      await send(JSON.stringify(notification));
    },

    async end(fullText: string) {
      // Callers that streamed via delta() may close with end("") — collapse to
      // the accumulated stream text instead of sending a blank message.
      const text = fullText || accumulated;
      const collapsed = {
        id: messageId,
        from: fromIdentity,
        thread_id: threadId,
        timestamp: new Date().toISOString(),
        type: "message",
        content: [{
          type: "text",
          version: "1.0",
          source: "agent",
          text,
          fallback: text,
        }],
      };

      const notification = buildNotification("ainm.stream.end", {
        stream_id: streamId,
        status: "completed",
        collapsed_message: collapsed,
      });
      await send(JSON.stringify(notification));
    },
  };
}
