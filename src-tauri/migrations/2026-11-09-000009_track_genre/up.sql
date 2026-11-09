-- the genre a file names, kept so the tag editor can offer the genres already in the library;
-- every file is read again to fill it
ALTER TABLE tracks ADD COLUMN genre TEXT;
UPDATE tracks SET modified = 0;
