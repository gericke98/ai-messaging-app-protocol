import type { MemoryContext, MemoryNodeInput } from "../protocol/types.js";

type RpcSender = (
  method: string,
  params: Record<string, unknown>
) => Promise<Record<string, unknown>>;

export function createMemoryContext(
  sendRpc: RpcSender,
  threadId: string,
  userIdentity: string
): MemoryContext {
  return {
    async getBriefing() {
      return sendRpc("ainm.memory.briefing", {
        thread_id: threadId,
        user_identity: userIdentity,
      });
    },

    async queryGraph(query?: string, limit?: number) {
      const params: Record<string, unknown> = { include_edges: true };
      if (query) params.query = query;
      if (limit) params.limit = limit;
      return sendRpc("ainm.memory.graph.query", params);
    },

    async contribute(node: MemoryNodeInput): Promise<string> {
      const nodePayload: Record<string, unknown> = {
        type: node.type,
        category: node.category,
        label: node.label,
        confidence: node.confidence,
      };
      if (node.properties) nodePayload.properties = node.properties;
      if (node.tags) nodePayload.tags = node.tags;

      const result = await sendRpc("ainm.memory.graph.contribute", {
        thread_id: threadId,
        node: nodePayload,
      });
      return result.node_id as string;
    },
  };
}
