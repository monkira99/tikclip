DROP INDEX IF EXISTS idx_recordings_sidecar_recording_id;

ALTER TABLE recordings RENAME COLUMN sidecar_recording_id TO external_recording_id;

CREATE UNIQUE INDEX IF NOT EXISTS idx_recordings_external_recording_id
  ON recordings(external_recording_id) WHERE external_recording_id IS NOT NULL;
