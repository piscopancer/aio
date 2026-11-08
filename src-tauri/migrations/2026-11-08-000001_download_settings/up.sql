-- A single row (id 1) with the download options: the default folder and when the tools were last updated.

CREATE TABLE download_settings (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  folder TEXT,
  tools_checked_at BIGINT
);
