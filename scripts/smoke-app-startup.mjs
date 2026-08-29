#!/usr/bin/env node

import { spawn } from "node:child_process";
import { access } from "node:fs/promises";
import { resolve } from "node:path";

// pnpm forwards the "--" separator to the script, so drop it here.
const [executablePath] = process.argv.slice(2).filter((argument) => argument !== "--");
const executable = executablePath ? resolve(executablePath) : null;
const waitMs = Number(process.env.QMO_STARTUP_SMOKE_MS ?? 4000);

if (!executable) {
  throw new Error("Pass the path to the built application executable.");
}
if (!Number.isFinite(waitMs) || waitMs < 500) {
  throw new Error("QMO_STARTUP_SMOKE_MS must be at least 500 milliseconds.");
}

await access(executable);

const child = spawn(executable, [], {
  env: { ...process.env, RUST_BACKTRACE: "1" },
  stdio: ["ignore", "pipe", "pipe"],
  windowsHide: true,
});

let stdout = "";
let stderr = "";
child.stdout.setEncoding("utf8");
child.stderr.setEncoding("utf8");
child.stdout.on("data", (chunk) => {
  stdout += chunk;
});
child.stderr.on("data", (chunk) => {
  stderr += chunk;
});

const result = await Promise.race([
  new Promise((resolveExit, rejectExit) => {
    child.once("error", rejectExit);
    child.once("exit", (code, signal) =>
      resolveExit({ exited: true, code, signal }),
    );
  }),
  new Promise((resolveWait) => {
    setTimeout(() => resolveWait({ exited: false }), waitMs);
  }),
]);

if (result.exited) {
  throw new Error(
    [
      `Application exited during the ${waitMs} ms startup smoke test`,
      `exit code: ${result.code ?? "none"}`,
      `signal: ${result.signal ?? "none"}`,
      stdout.trim() ? `stdout:\n${stdout.trim()}` : null,
      stderr.trim() ? `stderr:\n${stderr.trim()}` : null,
    ]
      .filter(Boolean)
      .join("\n"),
  );
}

child.kill();
await new Promise((resolveExit) => {
  if (child.exitCode !== null || child.signalCode !== null) {
    resolveExit();
    return;
  }
  child.once("exit", resolveExit);
  setTimeout(resolveExit, 2000);
});

process.stdout.write(`Application stayed alive for ${waitMs} ms.\n`);
