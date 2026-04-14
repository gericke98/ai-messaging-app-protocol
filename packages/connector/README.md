# @ainm/connector

TypeScript SDK for connecting AI agents to the AINM messaging network. Any agent runtime can become an AINM participant in 5 lines of code.

## Install

```bash
npm install @ainm/connector
```

## Quick Start

```typescript
import { AINMAgent } from "@ainm/connector";

const agent = new AINMAgent({
  server: "wss://my-server:8080/ws",
  identity: "my-agent@server.local",
  name: "My Agent",
  capabilities: ["text", "code"],
});

agent.onMessage(async (message, ctx) => {
  return `You said: ${message.text}`;
});

agent.connect();
```

## Streaming Responses

```typescript
agent.onMessage(async (message, ctx) => {
  const stream = ctx.stream();
  for (const word of message.text.split(" ")) {
    stream.delta(word + " ");
  }
  stream.end();
});
```

## Compound Memory

Pull the user's knowledge graph for context. Contribute back to build shared memory.

```typescript
agent.onMessage(async (message, ctx) => {
  // Get a briefing about the user (preferences, skills, history)
  const briefing = await ctx.memory.getBriefing();

  // Search the knowledge graph
  const results = await ctx.memory.queryGraph("preferences");

  // Contribute a new fact
  await ctx.memory.contribute({
    type: "semantic",
    category: "fact",
    label: "User prefers TypeScript",
    confidence: 0.9,
  });
});
```

## Tool Registration

Declare tools with optional approval gates for dangerous operations.

```typescript
agent.registerTool(
  "search",
  {
    description: "Search documentation",
    parameters: { query: { type: "string" } },
  },
  async (params, ctx) => {
    ctx.emitToolStarted("search", params);
    const results = await searchDocs(params.query as string);
    ctx.emitToolCompleted("search", results);
    return results;
  }
);
```

## Custom Transport

The SDK uses WebSocket by default. You can provide a custom transport:

```typescript
import { AINMAgent, type Transport } from "@ainm/connector";

class MyTransport implements Transport {
  // ... implement connect, send, onMessage, onClose, onError, close, isConnected
}

const agent = new AINMAgent(config, new MyTransport());
```

## Protocol

The SDK implements the [AINM Protocol](https://github.com/user/ai-native-messaging) over WebSocket with JSON-RPC 2.0 framing and Ed25519 message signing.

## License

MIT
