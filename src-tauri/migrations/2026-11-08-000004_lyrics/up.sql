-- what Aio knows about a song's lyrics beyond its file: the find, the recording length it was timed for, and whether
-- the file holds it yet; "missing" remembers a fruitless lookup for a while
CREATE TABLE lyrics (
  track_id INTEGER PRIMARY KEY NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('text', 'instrumental', 'missing')),
  text TEXT,
  lrclib_id INTEGER,
  length_ms INTEGER NOT NULL,
  in_file BOOLEAN NOT NULL,
  checked_at BIGINT NOT NULL
);
