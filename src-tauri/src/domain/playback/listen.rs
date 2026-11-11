use std::time::Duration;

use super::PlaybackContext;
use crate::domain::ids::TrackId;

/// One stretch of a song being played, from the moment the player takes it up until another song replaces it or the
/// player stops. It counts as a play once enough of it was heard; how long it was heard counts either way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listen {
  track: TrackId,
  context: Option<PlaybackContext>,
  /// Unix seconds.
  started_at: i64,
  length: Duration,
  heard: Duration,
}

impl Listen {
  /// How much of a song makes a play, unless the song is short enough that half of it is less.
  pub const ENOUGH: Duration = Duration::from_secs(30);

  pub fn new(track: TrackId, context: Option<PlaybackContext>, length: Duration, started_at: i64) -> Self {
    Self {
      track,
      context,
      started_at,
      length,
      heard: Duration::ZERO,
    }
  }

  /// Adds a span the song was heard for; true when that span made it count as a play.
  pub fn hear(&mut self, span: Duration) -> bool {
    let counted = self.counts();
    self.heard += span;
    !counted && self.counts()
  }

  /// Whether enough of the song was heard: thirty seconds, or half of a song shorter than a minute.
  pub fn counts(&self) -> bool {
    !self.heard.is_zero() && self.heard >= Self::ENOUGH.min(self.length / 2)
  }

  pub fn track(&self) -> TrackId {
    self.track
  }

  pub fn context(&self) -> Option<PlaybackContext> {
    self.context
  }

  pub fn started_at(&self) -> i64 {
    self.started_at
  }

  pub fn heard(&self) -> Duration {
    self.heard
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn listen(length_secs: u64) -> Listen {
    Listen::new(TrackId(1), None, Duration::from_secs(length_secs), 0)
  }

  #[test]
  fn a_song_counts_after_thirty_seconds() {
    let mut listen = listen(240);
    assert!(!listen.hear(Duration::from_secs(29)));
    assert!(listen.hear(Duration::from_secs(1)));
    // it counts once, however long it plays on
    assert!(!listen.hear(Duration::from_secs(60)));
    assert!(listen.counts());
    assert_eq!(listen.heard(), Duration::from_secs(90));
  }

  #[test]
  fn a_short_song_counts_after_half_of_it() {
    let mut listen = listen(40);
    assert!(!listen.hear(Duration::from_secs(19)));
    assert!(listen.hear(Duration::from_secs(1)));
  }

  #[test]
  fn an_unheard_song_never_counts() {
    assert!(!listen(0).counts());
  }
}
