-- when playback last started from a playlist, for ordering the sidebar by what was played recently; never played is null
ALTER TABLE playlists ADD COLUMN played_at BIGINT;
