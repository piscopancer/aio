-- A single row (id 1) with how the window behaves: whether closing it hides it to the tray, and whether a launch by
-- Windows at sign-in stays in the tray without showing it.
CREATE TABLE window_settings (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  close_to_tray BOOLEAN NOT NULL DEFAULT 1,
  start_hidden BOOLEAN NOT NULL DEFAULT 0
);
