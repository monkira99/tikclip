type SidecarRecordingLike = {
  recording_id?: unknown;
  account_id?: unknown;
};

export async function syncRecordingListFromSidecar<T extends SidecarRecordingLike>(
  rows: ReadonlyArray<T>,
  syncRecording: (payload: Record<string, unknown>) => Promise<void>,
): Promise<void> {
  for (const row of rows) {
    const recordingId = typeof row.recording_id === "string" ? row.recording_id : "";
    const accountId =
      typeof row.account_id === "number"
        ? row.account_id
        : typeof row.account_id === "string"
          ? Number(row.account_id)
          : NaN;
    if (!recordingId || !Number.isFinite(accountId) || accountId <= 0) {
      continue;
    }
    await syncRecording(row as Record<string, unknown>);
  }
}
