-- Initial schema of the music library, its playlists and the saved player options.

CREATE TABLE folders (
  id INTEGER PRIMARY KEY NOT NULL,
  path TEXT NOT NULL UNIQUE
);

CREATE TABLE artists (
  id INTEGER PRIMARY KEY NOT NULL,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE albums (
  id INTEGER PRIMARY KEY NOT NULL,
  title TEXT NOT NULL COLLATE NOCASE,
  artist_id INTEGER NOT NULL REFERENCES artists (id),
  year INTEGER,
  cover TEXT,
  UNIQUE (title, artist_id)
);

CREATE TABLE tracks (
  id INTEGER PRIMARY KEY NOT NULL,
  folder_id INTEGER NOT NULL REFERENCES folders (id) ON DELETE CASCADE,
  path TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  artist_id INTEGER NOT NULL REFERENCES artists (id),
  album_id INTEGER NOT NULL REFERENCES albums (id),
  track_no INTEGER,
  disc_no INTEGER,
  duration_ms INTEGER NOT NULL,
  modified BIGINT NOT NULL
);

CREATE INDEX tracks_album ON tracks (album_id);
CREATE INDEX tracks_artist ON tracks (artist_id);
CREATE INDEX tracks_folder ON tracks (folder_id);

CREATE TABLE playlists (
  id INTEGER PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  created_at BIGINT NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE playlist_tracks (
  playlist_id INTEGER NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
  track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  PRIMARY KEY (playlist_id, track_id)
);

-- A single row (id 1) with the options the player restores at startup.
CREATE TABLE playback_settings (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  volume INTEGER NOT NULL CHECK (volume BETWEEN 0 AND 100),
  shuffle BOOLEAN NOT NULL,
  repeat TEXT NOT NULL CHECK (repeat IN ('off', 'all', 'one'))
);
