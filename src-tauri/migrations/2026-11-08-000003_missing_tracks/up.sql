-- A track whose file left the library keeps its row, with no folder, while a playlist still holds it, so the
-- playlist shows it as missing and gets it back when the file returns. SQLite cannot relax a column in place, so
-- the tracks table is rebuilt; playlist_tracks is set aside first, since dropping tracks would cascade into it.

CREATE TABLE playlist_tracks_saved AS SELECT playlist_id, track_id, position FROM playlist_tracks;
DROP TABLE playlist_tracks;

CREATE TABLE tracks_new (
  id INTEGER PRIMARY KEY NOT NULL,
  folder_id INTEGER REFERENCES folders (id) ON DELETE SET NULL,
  path TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  artist_id INTEGER NOT NULL REFERENCES artists (id),
  album_id INTEGER NOT NULL REFERENCES albums (id),
  track_no INTEGER,
  disc_no INTEGER,
  duration_ms INTEGER NOT NULL,
  modified BIGINT NOT NULL
);
INSERT INTO tracks_new SELECT id, folder_id, path, title, artist_id, album_id, track_no, disc_no, duration_ms, modified
FROM tracks;
DROP TABLE tracks;
ALTER TABLE tracks_new RENAME TO tracks;

CREATE INDEX tracks_album ON tracks (album_id);
CREATE INDEX tracks_artist ON tracks (artist_id);
CREATE INDEX tracks_folder ON tracks (folder_id);

CREATE TABLE playlist_tracks (
  playlist_id INTEGER NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
  track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  PRIMARY KEY (playlist_id, track_id)
);
INSERT INTO playlist_tracks SELECT playlist_id, track_id, position FROM playlist_tracks_saved;
DROP TABLE playlist_tracks_saved;
