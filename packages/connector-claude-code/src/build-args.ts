export interface ClaudeArgsOptions {
  prompt: string;
  sessionId?: string;
  permissionMode?: string; // default: "auto"
  maxBudgetUsd?: number;
  bare?: boolean;
}

/**
 * Build the CLI argument array for spawning the `claude` binary.
 *
 * Arg order:
 *   1. -p <prompt>
 *   2. --output-format stream-json
 *   3. --verbose
 *   4. --include-partial-messages
 *   5. --permission-mode <mode>   (default: "auto")
 *   6. --resume <sessionId>       (if provided)
 *   7. --max-budget-usd <amount>  (if provided)
 *   8. --bare                     (if true)
 */
export function buildClaudeArgs(opts: ClaudeArgsOptions): string[] {
  const { prompt, sessionId, permissionMode = "auto", maxBudgetUsd, bare } = opts;

  const args: string[] = [
    "-p",
    prompt,
    "--output-format",
    "stream-json",
    "--verbose",
    "--include-partial-messages",
    "--permission-mode",
    permissionMode,
  ];

  if (sessionId !== undefined) {
    args.push("--resume", sessionId);
  }

  if (maxBudgetUsd !== undefined) {
    args.push("--max-budget-usd", String(maxBudgetUsd));
  }

  if (bare === true) {
    args.push("--bare");
  }

  return args;
}
