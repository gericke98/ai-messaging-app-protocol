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
      const notification = buildNotification("ainm.stream.delta", {
        stream_id: streamId,
        block_index: 0,
        delta: text,
      });
      await send(JSON.stringify(notification));
    },

    async end(fullText: string) {
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
          text: fullText,
          fallback: fullText,
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
