-- Songs fade in and out by default, while the crossfade between songs, in whole seconds, starts off.
ALTER TABLE playback_settings ADD COLUMN fades BOOLEAN NOT NULL DEFAULT 1;
ALTER TABLE playback_settings ADD COLUMN crossfade_seconds INTEGER NOT NULL DEFAULT 0;
