-- every artist a song credits, in order; tracks.artist_id stays as the first of them, which sorts the library
CREATE TABLE track_artists (
  track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
  artist_id INTEGER NOT NULL REFERENCES artists (id),
  position INTEGER NOT NULL,
  PRIMARY KEY (track_id, artist_id)
);

CREATE INDEX track_artists_artist ON track_artists (artist_id);

INSERT INTO track_artists (track_id, artist_id, position) SELECT id, artist_id, 0 FROM tracks;

-- the next scan reads every file again, so credits like "A feat. B" split into their artists
UPDATE tracks SET modified = 0;
