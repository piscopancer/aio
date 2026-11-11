-- every stretch of a song the player played, kept for good as the ground of the home page and later statistics:
-- where it was started from, when, how long it was actually heard, and whether that was enough to count as a play
CREATE TABLE plays (
  id INTEGER PRIMARY KEY NOT NULL,
  track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
  context_kind TEXT CHECK (context_kind IN ('songs', 'album', 'artist', 'playlist', 'folder', 'recent', 'added')),
  context_id INTEGER,
  started_at BIGINT NOT NULL,
  heard_ms INTEGER NOT NULL,
  counted BOOLEAN NOT NULL
);
CREATE INDEX plays_started_at ON plays (started_at);
CREATE INDEX plays_track_id ON plays (track_id);
