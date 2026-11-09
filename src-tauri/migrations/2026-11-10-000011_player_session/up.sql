-- What the player was doing when the app last ran, which the next launch restores paused: the queue as JSON (the
-- songs in play order, the original order of a shuffled queue and the current place) and the position in the current
-- song. A single row, id 1.
CREATE TABLE player_session (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  queue TEXT NOT NULL,
  position_ms INTEGER NOT NULL
);
