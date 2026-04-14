import { describe, it, expect } from "vitest";
import { buildRequest, buildNotification, parseMessage } from "../src/protocol/rpc.js";

describe("JSON-RPC builders", () => {
  it("builds a request with incrementing ID", () => {
    const req = buildRequest("ainm.initialize", { identity: "test" });
    expect(req.jsonrpc).toBe("2.0");
    expect(req.method).toBe("ainm.initialize");
    expect(req.params.identity).toBe("test");
    expect(typeof req.id).toBe("number");
  });

  it("builds a notification (no id)", () => {
    const notif = buildNotification("ainm.initialized", {});
    expect(notif.jsonrpc).toBe("2.0");
    expect(notif.method).toBe("ainm.initialized");
    expect(notif.id).toBeUndefined();
  });

  it("parses a response", () => {
    const raw = JSON.stringify({ jsonrpc: "2.0", id: 1, result: { protocolVersion: "0.1.0" } });
    const msg = parseMessage(raw);
    expect(msg.type).toBe("response");
    if (msg.type === "response") {
      expect(msg.id).toBe(1);
      expect(msg.result.protocolVersion).toBe("0.1.0");
    }
  });

  it("parses a notification", () => {
    const raw = JSON.stringify({ jsonrpc: "2.0", method: "ainm.message.deliver", params: { message: { text: "hello" } } });
    const msg = parseMessage(raw);
    expect(msg.type).toBe("notification");
    if (msg.type === "notification") {
      expect(msg.method).toBe("ainm.message.deliver");
    }
  });

  it("parses an error response", () => {
    const raw = JSON.stringify({ jsonrpc: "2.0", id: 2, error: { code: -32601, message: "Method not found" } });
    const msg = parseMessage(raw);
    expect(msg.type).toBe("error");
  });
});
