-- A song whose file names no artist is credited to nobody, and an album whose files name no artist belongs to nobody:
-- the "Unknown Artist" the library made for such songs goes, with every credit and album that named it. SQLite cannot
-- relax a column in place, so the albums and tracks tables are rebuilt; the tables that refer to tracks are set aside
-- first, since dropping tracks would cascade into them. The new tracks table refers to the new albums table under its
-- temporary name, which the rename at the end carries over.

CREATE TABLE playlist_tracks_saved AS SELECT playlist_id, track_id, position FROM playlist_tracks;
CREATE TABLE track_artists_saved AS SELECT track_id, artist_id, position FROM track_artists
  WHERE artist_id NOT IN (SELECT id FROM artists WHERE name = 'Unknown Artist');
CREATE TABLE lyrics_saved AS SELECT track_id, kind, text, lrclib_id, length_ms, in_file, checked_at FROM lyrics;
DROP TABLE playlist_tracks;
DROP TABLE track_artists;
DROP TABLE lyrics;

-- an album with no artist is told apart from the others by its title alone
CREATE TABLE albums_new (
  id INTEGER PRIMARY KEY NOT NULL,
  title TEXT NOT NULL COLLATE NOCASE,
  artist_id INTEGER REFERENCES artists (id),
  year INTEGER,
  cover TEXT,
  pin INTEGER
);
INSERT INTO albums_new
SELECT id, title,
  CASE WHEN artist_id IN (SELECT id FROM artists WHERE name = 'Unknown Artist') THEN NULL ELSE artist_id END,
  year, cover, pin
FROM albums;

CREATE TABLE tracks_new (
  id INTEGER PRIMARY KEY NOT NULL,
  folder_id INTEGER REFERENCES folders (id) ON DELETE SET NULL,
  path TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  artist_id INTEGER REFERENCES artists (id),
  album_id INTEGER REFERENCES albums_new (id),
  track_no INTEGER,
  disc_no INTEGER,
  duration_ms INTEGER NOT NULL,
  modified BIGINT NOT NULL,
  genre TEXT,
  cover TEXT
);
INSERT INTO tracks_new
SELECT id, folder_id, path, title,
  CASE WHEN artist_id IN (SELECT id FROM artists WHERE name = 'Unknown Artist') THEN NULL ELSE artist_id END,
  album_id, track_no, disc_no, duration_ms, modified, genre, cover
FROM tracks;
DROP TABLE tracks;
DROP TABLE albums;
ALTER TABLE albums_new RENAME TO albums;
ALTER TABLE tracks_new RENAME TO tracks;

CREATE UNIQUE INDEX albums_key ON albums (title, ifnull(artist_id, 0));
CREATE INDEX tracks_album ON tracks (album_id);
CREATE INDEX tracks_artist ON tracks (artist_id);
CREATE INDEX tracks_folder ON tracks (folder_id);

DELETE FROM artists WHERE name = 'Unknown Artist';

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
