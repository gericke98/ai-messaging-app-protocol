import { describe, it, expect } from "vitest";
import { createStreamHelper } from "../src/protocol/streaming.js";

describe("StreamHelper", () => {
  it("sends stream start, deltas, and end", async () => {
    const sent: string[] = [];
    const sendFn = async (data: string) => { sent.push(data); };

    const stream = createStreamHelper(sendFn, "agent@test", "thread-1");
    await stream.start();

    expect(sent.length).toBe(1);
    const start = JSON.parse(sent[0]);
    expect(start.method).toBe("ainm.stream.start");
    expect(start.params.thread_id).toBe("thread-1");
    expect(start.params.from).toBe("agent@test");
    expect(start.params.stream_id).toBeDefined();

    await stream.delta("Hello ");
    await stream.delta("world!");

    expect(sent.length).toBe(3);
    const d1 = JSON.parse(sent[1]);
    expect(d1.method).toBe("ainm.stream.delta");
    expect(d1.params.delta).toBe("Hello ");
    expect(d1.params.block_index).toBe(0);

    await stream.end("Hello world!");

    expect(sent.length).toBe(4);
    const end = JSON.parse(sent[3]);
    expect(end.method).toBe("ainm.stream.end");
    expect(end.params.status).toBe("completed");
    expect(end.params.collapsed_message).toBeDefined();
    expect(end.params.collapsed_message.content[0].text).toBe("Hello world!");
  });
});
