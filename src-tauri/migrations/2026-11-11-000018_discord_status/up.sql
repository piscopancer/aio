-- Discord shows the playing song on the user's profile unless they turn it off.
ALTER TABLE playback_settings ADD COLUMN discord_status BOOLEAN NOT NULL DEFAULT 1;
