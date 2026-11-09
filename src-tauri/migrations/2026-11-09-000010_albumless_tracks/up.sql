-- A song whose file names no album belongs to none: the "Unknown Album" rows the library made for such songs go, and
-- each song keeps the cover of its own file, which stands in for an album's. SQLite cannot relax a column in place,
-- so the tracks table is rebuilt; the tables that refer to it are set aside first, since dropping tracks would
-- cascade into them. Every file is read again to fill the covers.

CREATE TABLE playlist_tracks_saved AS SELECT playlist_id, track_id, position FROM playlist_tracks;
CREATE TABLE track_artists_saved AS SELECT track_id, artist_id, position FROM track_artists;
CREATE TABLE lyrics_saved AS SELECT track_id, kind, text, lrclib_id, length_ms, in_file, checked_at FROM lyrics;
DROP TABLE playlist_tracks;
DROP TABLE track_artists;
DROP TABLE lyrics;

CREATE TABLE tracks_new (
  id INTEGER PRIMARY KEY NOT NULL,
  folder_id INTEGER REFERENCES folders (id) ON DELETE SET NULL,
  path TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  artist_id INTEGER NOT NULL REFERENCES artists (id),
  album_id INTEGER REFERENCES albums (id),
  track_no INTEGER,
  disc_no INTEGER,
  duration_ms INTEGER NOT NULL,
  modified BIGINT NOT NULL,
  genre TEXT,
  cover TEXT
);
INSERT INTO tracks_new
SELECT id, folder_id, path, title, artist_id,
  CASE WHEN album_id IN (SELECT id FROM albums WHERE title = 'Unknown Album') THEN NULL ELSE album_id END,
  track_no, disc_no, duration_ms, 0, genre, NULL
FROM tracks;
DROP TABLE tracks;
ALTER TABLE tracks_new RENAME TO tracks;

CREATE INDEX tracks_album ON tracks (album_id);
CREATE INDEX tracks_artist ON tracks (artist_id);
CREATE INDEX tracks_folder ON tracks (folder_id);

DELETE FROM albums WHERE title = 'Unknown Album';

CREATE TABLE playlist_tracks (
  playlist_id INTEGER NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
  track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  PRIMARY KEY (playlist_id, track_id)
);
INSERT INTO playlist_tracks SELECT playlist_id, track_id, position FROM playlist_tracks_saved;
DROP TABLE playlist_tracks_saved;

CREATE TABLE track_artists (
  track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
  artist_id INTEGER NOT NULL REFERENCES artists (id),
  position INTEGER NOT NULL,
  PRIMARY KEY (track_id, artist_id)
);
CREATE INDEX track_artists_artist ON track_artists (artist_id);
INSERT INTO track_artists SELECT track_id, artist_id, position FROM track_artists_saved;
DROP TABLE track_artists_saved;

CREATE TABLE lyrics (
  track_id INTEGER PRIMARY KEY NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('text', 'instrumental', 'missing')),
  text TEXT,
  lrclib_id INTEGER,
  length_ms INTEGER NOT NULL,
  in_file BOOLEAN NOT NULL,
  checked_at BIGINT NOT NULL
);
INSERT INTO lyrics SELECT track_id, kind, text, lrclib_id, length_ms, in_file, checked_at FROM lyrics_saved;
DROP TABLE lyrics_saved;
