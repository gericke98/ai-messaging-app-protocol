import { describe, it, expect } from "vitest";
import { buildClaudeArgs } from "../src/build-args.js";

describe("buildClaudeArgs", () => {
  describe("default args", () => {
    it("includes -p followed by the prompt string", () => {
      const args = buildClaudeArgs({ prompt: "hello world" });
      expect(args[0]).toBe("-p");
      expect(args[1]).toBe("hello world");
    });

    it("includes --output-format stream-json", () => {
      const args = buildClaudeArgs({ prompt: "test" });
      const idx = args.indexOf("--output-format");
      expect(idx).not.toBe(-1);
      expect(args[idx + 1]).toBe("stream-json");
    });

    it("includes --verbose", () => {
      const args = buildClaudeArgs({ prompt: "test" });
      expect(args).toContain("--verbose");
    });

    it("includes --include-partial-messages", () => {
      const args = buildClaudeArgs({ prompt: "test" });
      expect(args).toContain("--include-partial-messages");
    });

    it("includes --permission-mode auto by default", () => {
      const args = buildClaudeArgs({ prompt: "test" });
      const idx = args.indexOf("--permission-mode");
      expect(idx).not.toBe(-1);
      expect(args[idx + 1]).toBe("auto");
    });

    it("does not include --resume when sessionId is not provided", () => {
      const args = buildClaudeArgs({ prompt: "test" });
      expect(args).not.toContain("--resume");
    });

    it("does not include --max-budget-usd when maxBudgetUsd is not provided", () => {
      const args = buildClaudeArgs({ prompt: "test" });
      expect(args).not.toContain("--max-budget-usd");
    });

    it("does not include --bare by default", () => {
      const args = buildClaudeArgs({ prompt: "test" });
      expect(args).not.toContain("--bare");
    });
  });

  describe("arg ordering", () => {
    it("produces args in the required order for a minimal call", () => {
      const args = buildClaudeArgs({ prompt: "my prompt" });
      expect(args).toEqual([
        "-p",
        "my prompt",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--permission-mode",
        "auto",
      ]);
    });
  });

  describe("sessionId", () => {
    it("adds --resume <id> when sessionId is provided", () => {
      const args = buildClaudeArgs({ prompt: "test", sessionId: "abc-123" });
      const idx = args.indexOf("--resume");
      expect(idx).not.toBe(-1);
      expect(args[idx + 1]).toBe("abc-123");
    });

    it("places --resume after the default flags", () => {
      const args = buildClaudeArgs({ prompt: "test", sessionId: "sess-1" });
      const permIdx = args.indexOf("--permission-mode");
      const resumeIdx = args.indexOf("--resume");
      expect(resumeIdx).toBeGreaterThan(permIdx);
    });
  });

  describe("maxBudgetUsd", () => {
    it("adds --max-budget-usd <amount> when maxBudgetUsd is provided", () => {
      const args = buildClaudeArgs({ prompt: "test", maxBudgetUsd: 5 });
      const idx = args.indexOf("--max-budget-usd");
      expect(idx).not.toBe(-1);
      expect(args[idx + 1]).toBe("5");
    });

    it("places --max-budget-usd after the default flags", () => {
      const args = buildClaudeArgs({ prompt: "test", maxBudgetUsd: 10 });
      const permIdx = args.indexOf("--permission-mode");
      const budgetIdx = args.indexOf("--max-budget-usd");
      expect(budgetIdx).toBeGreaterThan(permIdx);
    });

    it("handles decimal budget values", () => {
      const args = buildClaudeArgs({ prompt: "test", maxBudgetUsd: 1.5 });
      const idx = args.indexOf("--max-budget-usd");
      expect(args[idx + 1]).toBe("1.5");
    });
  });

  describe("permissionMode", () => {
    it("overrides the default permission mode when specified", () => {
      const args = buildClaudeArgs({ prompt: "test", permissionMode: "bypassPermissions" });
      const idx = args.indexOf("--permission-mode");
      expect(idx).not.toBe(-1);
      expect(args[idx + 1]).toBe("bypassPermissions");
    });

    it("still has only one --permission-mode flag when overridden", () => {
      const args = buildClaudeArgs({ prompt: "test", permissionMode: "default" });
      const occurrences = args.filter((a) => a === "--permission-mode").length;
      expect(occurrences).toBe(1);
    });
  });

  describe("bare flag", () => {
    it("adds --bare when bare is true", () => {
      const args = buildClaudeArgs({ prompt: "test", bare: true });
      expect(args).toContain("--bare");
    });

    it("does not add --bare when bare is false", () => {
      const args = buildClaudeArgs({ prompt: "test", bare: false });
      expect(args).not.toContain("--bare");
    });

    it("places --bare at the end of the arg list", () => {
      const args = buildClaudeArgs({ prompt: "test", bare: true });
      expect(args[args.length - 1]).toBe("--bare");
    });
  });

  describe("combined options", () => {
    it("produces the full arg list with all options set", () => {
      const args = buildClaudeArgs({
        prompt: "do something",
        sessionId: "sess-42",
        permissionMode: "bypassPermissions",
        maxBudgetUsd: 2.5,
        bare: true,
      });
      expect(args).toEqual([
        "-p",
        "do something",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--permission-mode",
        "bypassPermissions",
        "--resume",
        "sess-42",
        "--max-budget-usd",
        "2.5",
        "--bare",
      ]);
    });

    it("works with sessionId and maxBudgetUsd but no bare flag", () => {
      const args = buildClaudeArgs({
        prompt: "test",
        sessionId: "s1",
        maxBudgetUsd: 3,
      });
      expect(args).toEqual([
        "-p",
        "test",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--permission-mode",
        "auto",
        "--resume",
        "s1",
        "--max-budget-usd",
        "3",
      ]);
    });
  });
});
