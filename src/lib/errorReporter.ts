import { invoke } from "@tauri-apps/api/core";

export interface ErrorEntry {
  timestamp: string;
  source: string;
  level: string;
  message: string;
  context?: Record<string, unknown> | null;
  stack?: string | null;
}

let initialized = false;

export async function reportError(
  message: string,
  context?: Record<string, unknown>,
  stack?: string,
): Promise<void> {
  try {
    await invoke("report_error", {
      source: "frontend",
      message,
      context: context ?? null,
      stack: stack ?? null,
    });
  } catch {
    console.error("[QMO]", message, context, stack);
  }
}

export async function getErrorLogPath(): Promise<string> {
  try {
    return await invoke<string>("get_error_log_path");
  } catch {
    return "logs/app-errors.jsonl";
  }
}

export async function getErrorLog(): Promise<ErrorEntry[]> {
  try {
    return await invoke<ErrorEntry[]>("get_error_log");
  } catch {
    return [];
  }
}

export async function clearErrorLog(): Promise<void> {
  try {
    await invoke("clear_error_log");
  } catch (error) {
    console.error("Failed to clear error log", error);
  }
}

export function initErrorReporting(): void {
  if (initialized || typeof window === "undefined") return;
  initialized = true;

  window.addEventListener("error", (event) => {
    void reportError(
      event.message || "Unknown window error",
      {
        filename: event.filename,
        lineno: event.lineno,
        colno: event.colno,
      },
      event.error?.stack,
    );
  });

  window.addEventListener("unhandledrejection", (event) => {
    const reason = event.reason;
    const message =
      reason instanceof Error
        ? reason.message
        : typeof reason === "string"
          ? reason
          : "Unhandled promise rejection";

    void reportError(
      message,
      { type: "unhandledrejection" },
      reason instanceof Error ? reason.stack : undefined,
    );
  });
}

export async function invokeLogged<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    const message = String(error);
    await reportError(message, { command, args: args ?? {} });
    throw error;
  }
}
