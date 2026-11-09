-- songs without an album keep none; only the covers of their files are dropped
ALTER TABLE tracks DROP COLUMN cover;
