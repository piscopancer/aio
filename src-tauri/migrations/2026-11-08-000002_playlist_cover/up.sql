-- an image the user picked for the playlist, stored in the covers folder; without one the songs' art stands in
ALTER TABLE playlists ADD COLUMN cover TEXT;
