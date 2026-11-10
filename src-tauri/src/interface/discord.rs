//! The playing song as the user's status in Discord, "Listening to Aio" with the song's title, artists and a
//! time bar; the status goes away while the player is paused or stopped, and when the user turns it off. Discord may
//! not run at all, so the status is sent from a thread of its own that keeps trying to reach it and never bothers the
//! player or the user.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, ActivityType, Assets, StatusDisplayType, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::application::views::PlayerState;

/// The Discord application the status belongs to, which gives it the name and the pictures.
const APP_ID: &str = "1569776823401848882";
/// The key of the app's logo among the pictures uploaded to the Discord application.
const LOGO: &str = "logo";
/// How long the sender waits for further changes before passing the last one on, since skipping through songs makes
/// many in a row and Discord takes only a few updates in a short while.
const SETTLE: Duration = Duration::from_secs(1);
/// How often Discord is looked for again while a status waits to be shown and Discord is not there.
const RETRY: Duration = Duration::from_secs(20);
/// How far the position may stray from where the shown time bar puts it before the bar is sent again, as after seeking.
const DRIFT: Duration = Duration::from_secs(2);
/// The shortest and the longest line Discord accepts, in characters.
const LINE: (usize, usize) = (2, 128);

/// The song the status shows, and the moment it would have started had it played without pause, which places the
/// time bar.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Listening {
  title: String,
  artists: String,
  duration_ms: u32,
  /// Unix milliseconds.
  started_at: i64,
}

impl Listening {
  /// The same song placed on the same time bar, give or take `DRIFT`.
  fn matches(&self, other: &Self) -> bool {
    let same_song = Self {
      started_at: other.started_at,
      ..self.clone()
    } == *other;
    same_song && self.drift(other.started_at) <= DRIFT
  }

  fn drift(&self, started_at: i64) -> Duration {
    Duration::from_millis(self.started_at.abs_diff(started_at))
  }
}

/// The status the player wants shown and the way to the thread that shows it.
struct Status {
  wanted: Mutex<Option<Listening>>,
  sender: Sender<Option<Listening>>,
}

static STATUS: OnceLock<Status> = OnceLock::new();

fn status() -> &'static Status {
  STATUS.get_or_init(|| {
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
      .name("discord".into())
      .spawn(move || work(receiver))
      .expect("the Discord status thread starts");
    Status {
      wanted: Mutex::default(),
      sender,
    }
  })
}

/// Shows the player's new state in Discord, from whichever thread reports it.
pub fn show(state: &PlayerState) {
  let track = state
    .index
    .and_then(|index| state.queue.get(usize::try_from(index).ok()?))
    .map(|item| &item.track)
    .filter(|_| state.discord_status && state.is_playing);
  let listening = track.map(|track| Listening {
    title: track.title.clone(),
    // the same credit line the player bar shows
    artists: track
      .artists
      .iter()
      .map(|artist| artist.name.as_str())
      .collect::<Vec<_>>()
      .join(", "),
    duration_ms: track.duration_ms,
    started_at: started_at(state.position_ms),
  });
  status().want(listening, |shown, next| shown.matches(next));
}

/// Moves the time bar when the position strays from it, as after seeking.
pub fn progress(position_ms: u32) {
  let Some(status) = STATUS.get() else { return };
  let started_at = started_at(position_ms);
  let next = status.wanted().clone().map(|shown| Listening { started_at, ..shown });
  status.want(next, |shown, next| shown.drift(next.started_at) <= DRIFT);
}

impl Status {
  fn wanted(&self) -> MutexGuard<'_, Option<Listening>> {
    // a poisoned lock only means an update panicked; the next one replaces what it held
    self.wanted.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  /// Passes `next` on unless `same` finds it shows nothing new.
  fn want(&self, next: Option<Listening>, same: impl Fn(&Listening, &Listening) -> bool) {
    let mut wanted = self.wanted();
    let unchanged = match (wanted.as_ref(), next.as_ref()) {
      (Some(shown), Some(next)) => same(shown, next),
      (None, None) => true,
      _ => false,
    };
    if !unchanged {
      *wanted = next.clone();
      // the thread only ends with the app
      let _ = self.sender.send(next);
    }
  }
}

/// Unix milliseconds of the moment a song now at `position_ms` started.
fn started_at(position_ms: u32) -> i64 {
  let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
  i64::try_from(
    now
      .saturating_sub(Duration::from_millis(position_ms.into()))
      .as_millis(),
  )
  .unwrap_or(i64::MAX)
}

/// Keeps Discord showing the last status wanted, connecting to it when it starts and again after it closes.
fn work(changes: Receiver<Option<Listening>>) {
  let mut client: Option<DiscordIpcClient> = None;
  let mut wanted = None;
  let mut shown = None;
  loop {
    // while a status waits for Discord, look for it every so often; otherwise wait for the player
    let change = if wanted == shown {
      changes.recv().map_err(|_| RecvTimeoutError::Disconnected)
    } else {
      changes.recv_timeout(RETRY)
    };
    match change {
      Ok(next) => wanted = next,
      Err(RecvTimeoutError::Timeout) => {}
      Err(RecvTimeoutError::Disconnected) => return,
    }
    let settled = Instant::now() + SETTLE;
    while let Ok(next) = changes.recv_timeout(settled.saturating_duration_since(Instant::now())) {
      wanted = next;
    }
    if wanted == shown {
      continue;
    }
    if client.is_none() {
      let mut fresh = DiscordIpcClient::new(APP_ID);
      // Discord not running is the usual case, not a failure worth reporting
      if fresh.connect().is_err() {
        continue;
      }
      client = Some(fresh);
    }
    let Some(connected) = client.as_mut() else { continue };
    match send(connected, wanted.as_ref()) {
      Ok(()) => shown = wanted.clone(),
      Err(error) => {
        // Discord closed; it shows nothing then, and the status is sent again once it is back
        eprintln!("Discord status not updated: {error}");
        client = None;
        shown = None;
      }
    }
  }
}

/// Sets or clears the status and reads Discord's reply, which also keeps the replies from piling up unread.
fn send(
  client: &mut DiscordIpcClient,
  listening: Option<&Listening>,
) -> Result<(), discord_rich_presence::error::Error> {
  match listening {
    Some(listening) => client.set_activity(activity(listening))?,
    None => client.clear_activity()?,
  }
  let (_, reply) = client.recv()?;
  if reply["evt"] == "ERROR" {
    // a status Discord turns down would be turned down again, so it is not retried
    eprintln!("Discord turned the status down: {}", reply["data"]["message"]);
  }
  Ok(())
}

fn activity(listening: &Listening) -> Activity<'static> {
  let mut timestamps = Timestamps::new().start(listening.started_at);
  if listening.duration_ms > 0 {
    timestamps = timestamps.end(listening.started_at.saturating_add(listening.duration_ms.into()));
  }
  let mut activity = Activity::new()
    .name("Aio")
    .activity_type(ActivityType::Listening)
    .status_display_type(StatusDisplayType::Name)
    .details(line(&listening.title))
    .timestamps(timestamps)
    .assets(Assets::new().large_image(LOGO));
  if !listening.artists.is_empty() {
    activity = activity.state(line(&listening.artists));
  }
  activity
}

/// `text` cut or padded to a length Discord accepts.
fn line(text: &str) -> String {
  let (shortest, longest) = LINE;
  let mut line: String = text.chars().take(longest).collect();
  let short = shortest.saturating_sub(line.chars().count());
  // a blank Braille pattern pads without showing, where Discord would trim a plain space
  line.extend(std::iter::repeat_n('\u{2800}', short));
  line
}

#[cfg(test)]
mod tests {
  use super::*;

  fn listening(started_at: i64) -> Listening {
    Listening {
      title: "Song".into(),
      artists: "Artist".into(),
      duration_ms: 200_000,
      started_at,
    }
  }

  #[test]
  fn same_song_within_drift_matches() {
    assert!(listening(10_000).matches(&listening(11_500)));
    assert!(!listening(10_000).matches(&listening(13_000)));
    let other = Listening {
      title: "Other".into(),
      ..listening(10_000)
    };
    assert!(!listening(10_000).matches(&other));
  }

  #[test]
  fn lines_fit_discord() {
    assert_eq!(line("A").chars().count(), 2);
    assert_eq!(line(&"x".repeat(300)).chars().count(), 128);
    assert_eq!(line("Song"), "Song");
  }
}
