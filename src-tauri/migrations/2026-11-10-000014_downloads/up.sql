-- The downloads the list shows, kept across launches until they are dismissed: what the user asked for as JSON, the
-- folder the song goes to, the file name picked once it first ran, so that a download cut short by quitting resumes
-- into the same partial file, and how it ended. A download still running when the app closed is resumed on launch.
CREATE TABLE downloads (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  request TEXT NOT NULL,
  folder TEXT NOT NULL,
  file_name TEXT,
  state TEXT NOT NULL DEFAULT 'running' CHECK (state IN ('running', 'done', 'failed')),
  track_id INTEGER,
  error TEXT,
  CHECK ((state = 'done') = (track_id IS NOT NULL)),
  CHECK ((state = 'failed') = (error IS NOT NULL))
);
