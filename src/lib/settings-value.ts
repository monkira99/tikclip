export function valueFromDb(db: string | null, fallback: string): string {
  return db === null ? fallback : db;
}

export function parseBooleanSetting(
  raw: string | null,
  defaultValue: boolean,
): boolean {
  if (raw === null || raw.trim() === "") {
    return defaultValue;
  }
  const t = raw.trim().toLowerCase();
  if (t === "1" || t === "true" || t === "yes" || t === "on") {
    return true;
  }
  if (t === "0" || t === "false" || t === "no" || t === "off") {
    return false;
  }
  return defaultValue;
}
