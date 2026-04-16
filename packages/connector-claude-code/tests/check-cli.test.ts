import { describe, it, expect } from "vitest";
import { checkClaudeAvailable } from "../src/check-cli.js";

describe("checkClaudeAvailable", () => {
  it("resolves with a version string when the binary exists", async () => {
    // This test requires the `claude` CLI to be installed.
    // It is skipped automatically when the binary is not available.
    let claudeInstalled = true;
    try {
      await checkClaudeAvailable("claude");
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      if (message.includes("not found")) {
        claudeInstalled = false;
      }
    }

    if (!claudeInstalled) {
      console.warn("Skipping: `claude` binary not found on this machine.");
      return;
    }

    const version = await checkClaudeAvailable("claude");
    expect(typeof version).toBe("string");
    expect(version.length).toBeGreaterThan(0);
    // claude --version returns something like "2.1.110 (Claude Code)"
    expect(version).toMatch(/\d+\.\d+/);
  });

  it("rejects with a clear error message when the binary does not exist", async () => {
    await expect(
      checkClaudeAvailable("nonexistent-binary-xyz")
    ).rejects.toThrow(
      "Claude Code CLI not found at 'nonexistent-binary-xyz'. Install from https://claude.ai/code"
    );
  });
});
