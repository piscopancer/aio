-- place among the pinned of its list, the first being 0; null when not pinned
ALTER TABLE playlists ADD COLUMN pin INTEGER;
ALTER TABLE albums ADD COLUMN pin INTEGER;
ALTER TABLE artists ADD COLUMN pin INTEGER;
