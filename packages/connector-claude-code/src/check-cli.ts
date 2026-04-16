import { spawn } from "node:child_process";

/**
 * Verify that the given binary is a working Claude Code CLI.
 *
 * Spawns `<binary> --version`, captures stdout, and returns the version
 * string (e.g. "2.1.110 (Claude Code)").
 *
 * Rejects with a user-friendly message when:
 *  - the binary is not found (ENOENT)
 *  - the spawn times out (5 s)
 *  - the process exits with a non-zero code (stderr forwarded)
 */
export function checkClaudeAvailable(binary: string): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    let stdout = "";
    let stderr = "";
    let settled = false;

    const child = spawn(binary, ["--version"], {
      stdio: ["ignore", "pipe", "pipe"],
    });

    const timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      child.kill();
      reject(
        new Error(
          `Claude Code CLI not found at '${binary}'. Install from https://claude.ai/code`
        )
      );
    }, 5_000);

    child.stdout.on("data", (chunk: Buffer) => {
      stdout += chunk.toString();
    });

    child.stderr.on("data", (chunk: Buffer) => {
      stderr += chunk.toString();
    });

    child.on("error", (err: NodeJS.ErrnoException) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);

      if (err.code === "ENOENT") {
        reject(
          new Error(
            `Claude Code CLI not found at '${binary}'. Install from https://claude.ai/code`
          )
        );
      } else {
        reject(err);
      }
    });

    child.on("close", (code: number | null) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);

      if (code !== 0) {
        reject(new Error(stderr.trim() || `${binary} exited with code ${code}`));
        return;
      }

      resolve(stdout.trim());
    });
  });
}
