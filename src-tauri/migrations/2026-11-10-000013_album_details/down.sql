ALTER TABLE tracks ADD COLUMN genre TEXT;
UPDATE tracks SET genre = (SELECT name FROM track_genres WHERE track_id = tracks.id ORDER BY position LIMIT 1);
DROP TABLE track_genres;
ALTER TABLE albums DROP COLUMN release_type;
DROP TABLE album_artists;
