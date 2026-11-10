-- An album may belong to several artists, as joint albums do, and says what kind of release it is; a song may name
-- several genres. albums.artist_id keeps the first album artist, which narrows the search for an album, as
-- tracks.artist_id keeps the main artist. Every file is read again to fill the new tables.

CREATE TABLE album_artists (
  album_id INTEGER NOT NULL REFERENCES albums (id) ON DELETE CASCADE,
  artist_id INTEGER NOT NULL REFERENCES artists (id),
  position INTEGER NOT NULL,
  PRIMARY KEY (album_id, artist_id)
);
CREATE INDEX album_artists_artist ON album_artists (artist_id);
INSERT INTO album_artists SELECT id, artist_id, 0 FROM albums WHERE artist_id IS NOT NULL;

ALTER TABLE albums ADD COLUMN release_type TEXT CHECK (release_type IN ('album', 'single', 'ep'));

CREATE TABLE track_genres (
  track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  position INTEGER NOT NULL,
  PRIMARY KEY (track_id, name)
);
CREATE INDEX track_genres_name ON track_genres (name);
INSERT INTO track_genres SELECT id, genre, 0 FROM tracks WHERE genre IS NOT NULL;
ALTER TABLE tracks DROP COLUMN genre;

UPDATE tracks SET modified = 0;
