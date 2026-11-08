-- Missing tracks have no folder to go back to, so they are dropped before the column is made required again.

DELETE FROM tracks WHERE folder_id IS NULL;

CREATE TABLE playlist_tracks_saved AS SELECT playlist_id, track_id, position FROM playlist_tracks;
DROP TABLE playlist_tracks;

CREATE TABLE tracks_old (
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
INSERT INTO tracks_old SELECT id, folder_id, path, title, artist_id, album_id, track_no, disc_no, duration_ms, modified
FROM tracks;
DROP TABLE tracks;
ALTER TABLE tracks_old RENAME TO tracks;

CREATE INDEX tracks_album ON tracks (album_id);
CREATE INDEX tracks_artist ON tracks (artist_id);
CREATE INDEX tracks_folder ON tracks (folder_id);

CREATE TABLE playlist_tracks (
  playlist_id INTEGER NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
  track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  PRIMARY KEY (playlist_id, track_id)
);
INSERT INTO playlist_tracks SELECT s.playlist_id, s.track_id, s.position FROM playlist_tracks_saved s
JOIN tracks t ON t.id = s.track_id;
DROP TABLE playlist_tracks_saved;
