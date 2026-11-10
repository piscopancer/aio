-- A hidden folder keeps its songs indexed, so showing it again is instant, while the library leaves them out and the
-- playlists that hold them show them dimmed.
ALTER TABLE folders ADD COLUMN hidden BOOLEAN NOT NULL DEFAULT 0;
