/** Tauri `invoke` rejections are often a string or a plain object, not `Error`. */
export function formatInvokeError(e: unknown): string {
  if (typeof e === "string" && e.trim().length > 0) {
    return e.trim();
  }
  if (e instanceof Error && e.message.trim().length > 0) {
    return e.message.trim();
  }
  if (e && typeof e === "object") {
    const o = e as Record<string, unknown>;
    const msg = o.message;
    if (typeof msg === "string" && msg.trim().length > 0) {
      return msg.trim();
    }
    const err = o.error;
    if (typeof err === "string" && err.trim().length > 0) {
      return err.trim();
    }
  }
  return "Unknown error";
}
