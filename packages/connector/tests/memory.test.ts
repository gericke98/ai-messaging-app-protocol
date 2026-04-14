import { describe, it, expect, vi } from "vitest";
import { createMemoryContext } from "../src/memory/context.js";

describe("MemoryContext", () => {
  it("requests a briefing via RPC", async () => {
    const sendRpc = vi.fn(async (method: string, params: Record<string, unknown>) => {
      return {
        briefing_id: "b-1",
        scope: "standard",
        sections: { identity: { name: "Test User" } },
      };
    });

    const mem = createMemoryContext(sendRpc, "thread-1", "user@test");
    const briefing = await mem.getBriefing();
    expect(sendRpc).toHaveBeenCalledWith("ainm.memory.briefing", {
      thread_id: "thread-1",
      user_identity: "user@test",
    });
    expect((briefing as any).briefing_id).toBe("b-1");
  });

  it("contributes a node to the graph", async () => {
    const sendRpc = vi.fn(async () => ({
      contributed: true,
      node_id: "node-123",
      edge_ids: [],
    }));

    const mem = createMemoryContext(sendRpc, "thread-1", "user@test");
    const nodeId = await mem.contribute({
      type: "semantic",
      category: "fact",
      label: "Likes Rust",
      confidence: 0.9,
    });
    expect(nodeId).toBe("node-123");
    expect(sendRpc).toHaveBeenCalledWith("ainm.memory.graph.contribute", {
      thread_id: "thread-1",
      node: {
        type: "semantic",
        category: "fact",
        label: "Likes Rust",
        confidence: 0.9,
      },
    });
  });

  it("queries the graph", async () => {
    const sendRpc = vi.fn(async () => ({
      nodes: [{ node_id: "n1", label: "Rust" }],
      edges: [],
      total_count: 1,
      returned_count: 1,
    }));

    const mem = createMemoryContext(sendRpc, "thread-1", "user@test");
    const result = await mem.queryGraph("Rust", 10);
    expect(sendRpc).toHaveBeenCalledWith("ainm.memory.graph.query", {
      query: "Rust",
      include_edges: true,
      limit: 10,
    });
    expect((result as any).nodes.length).toBe(1);
  });
});
