-- The database keeps only what the files cannot tell. A song's main artist is the first of its credits and an album's
-- first owner the first of its album artists, so the copies of them in tracks.artist_id and albums.artist_id go, and
-- an album is told apart by its title and its whole list of album artists, which the library checks, rather than by
-- a unique index on its first owner. Lyrics written into their song file are read from the file, so only the finds
-- still waiting to be written keep their text, and whether the file holds them becomes part of the kind.
--
-- SQLite cannot drop a column that refers to another table, so albums and tracks are rebuilt; the tables that refer to
-- them are set aside first, since dropping them would cascade into those. The new tracks table refers to the new
-- albums table under its temporary name, which the rename carries over.

CREATE TABLE playlist_tracks_saved AS SELECT playlist_id, track_id, position FROM playlist_tracks;
CREATE TABLE track_artists_saved AS SELECT track_id, artist_id, position FROM track_artists;
CREATE TABLE track_genres_saved AS SELECT track_id, name, position FROM track_genres;
CREATE TABLE album_artists_saved AS SELECT album_id, artist_id, position FROM album_artists;
CREATE TABLE lyrics_saved AS SELECT track_id, kind, text, lrclib_id, length_ms, in_file, checked_at FROM lyrics;
DROP TABLE playlist_tracks;
DROP TABLE track_artists;
DROP TABLE track_genres;
DROP TABLE album_artists;
DROP TABLE lyrics;

CREATE TABLE albums_new (
  id INTEGER PRIMARY KEY NOT NULL,
  title TEXT NOT NULL COLLATE NOCASE,
  year INTEGER,
  cover TEXT,
  pin INTEGER,
  release_type TEXT CHECK (release_type IN ('album', 'single', 'ep'))
);
INSERT INTO albums_new SELECT id, title, year, cover, pin, release_type FROM albums;

CREATE TABLE tracks_new (
  id INTEGER PRIMARY KEY NOT NULL,
  folder_id INTEGER REFERENCES folders (id) ON DELETE SET NULL,
  path TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  album_id INTEGER REFERENCES albums_new (id),
  track_no INTEGER,
  disc_no INTEGER,
  duration_ms INTEGER NOT NULL,
  modified BIGINT NOT NULL,
  cover TEXT
);
INSERT INTO tracks_new
SELECT id, folder_id, path, title, album_id, track_no, disc_no, duration_ms, modified, cover FROM tracks;
DROP TABLE tracks;
DROP TABLE albums;
ALTER TABLE albums_new RENAME TO albums;
ALTER TABLE tracks_new RENAME TO tracks;

CREATE INDEX albums_title ON albums (title);
CREATE INDEX tracks_album ON tracks (album_id);
CREATE INDEX tracks_folder ON tracks (folder_id);

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

CREATE TABLE track_genres (
  track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  position INTEGER NOT NULL,
  PRIMARY KEY (track_id, name)
);
CREATE INDEX track_genres_name ON track_genres (name);
INSERT INTO track_genres SELECT track_id, name, position FROM track_genres_saved;
DROP TABLE track_genres_saved;

CREATE TABLE album_artists (
  album_id INTEGER NOT NULL REFERENCES albums (id) ON DELETE CASCADE,
  artist_id INTEGER NOT NULL REFERENCES artists (id),
  position INTEGER NOT NULL,
  PRIMARY KEY (album_id, artist_id)
);
CREATE INDEX album_artists_artist ON album_artists (artist_id);
INSERT INTO album_artists SELECT album_id, artist_id, position FROM album_artists_saved;
DROP TABLE album_artists_saved;

-- "unwritten" finds wait with their text until their file is not playing; "written" ones only remember where they
-- came from, the file holding the text now
CREATE TABLE lyrics (
  track_id INTEGER PRIMARY KEY NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('unwritten', 'written', 'instrumental', 'missing')),
  text TEXT CHECK ((kind = 'unwritten') = (text IS NOT NULL)),
  lrclib_id INTEGER,
  length_ms INTEGER NOT NULL,
  checked_at BIGINT NOT NULL
);
INSERT INTO lyrics
SELECT track_id,
  CASE WHEN kind = 'text' AND in_file THEN 'written' WHEN kind = 'text' THEN 'unwritten' ELSE kind END,
  CASE WHEN kind = 'text' AND NOT in_file THEN text END,
  lrclib_id, length_ms, checked_at
FROM lyrics_saved
WHERE kind <> 'text' OR in_file OR text IS NOT NULL;
DROP TABLE lyrics_saved;
