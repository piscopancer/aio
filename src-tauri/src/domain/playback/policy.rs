use std::time::Duration;

use super::RepeatMode;
use super::queue::Advance;

/// Pressing "previous" later than this into a track restarts it instead of going back.
pub const RESTART_AFTER: Duration = Duration::from_secs(3);

/// How far before the saved place a song restored after a restart resumes, so the listener catches the moment again.
pub const RESUME_REWIND: Duration = Duration::from_secs(3);

/// Where a song restored after a restart resumes when playback stopped at `stopped`.
pub fn resume_point(stopped: Duration) -> Duration {
  stopped.saturating_sub(RESUME_REWIND)
}

/// Which entry plays after `current`, or `None` when playback reaches the end of the queue.
pub fn next_index(repeat: RepeatMode, current: usize, len: usize, reason: Advance) -> Option<usize> {
  match (repeat, reason) {
    (RepeatMode::One, Advance::TrackEnded) => Some(current),
    _ if current + 1 < len => Some(current + 1),
    (RepeatMode::All | RepeatMode::One, _) => Some(0),
    (RepeatMode::Off, _) => None,
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Previous {
  /// Start the current entry over.
  Restart,
  /// Play the entry at this index.
  Go(usize),
}

/// What "previous" does, given how far into the current track playback is.
pub fn previous(repeat: RepeatMode, current: usize, len: usize, elapsed: Duration) -> Previous {
  if elapsed >= RESTART_AFTER {
    return Previous::Restart;
  }
  match (current, repeat) {
    (0, RepeatMode::All) if len > 1 => Previous::Go(len - 1),
    (0, _) => Previous::Restart,
    (index, _) => Previous::Go(index - 1),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn next_stops_at_the_end_without_repeat() {
    assert_eq!(next_index(RepeatMode::Off, 1, 3, Advance::Skipped), Some(2));
    assert_eq!(next_index(RepeatMode::Off, 2, 3, Advance::TrackEnded), None);
  }

  #[test]
  fn next_wraps_with_repeat_all() {
    assert_eq!(next_index(RepeatMode::All, 2, 3, Advance::TrackEnded), Some(0));
  }

  #[test]
  fn repeat_one_replays_on_end_but_skips_on_demand() {
    assert_eq!(next_index(RepeatMode::One, 1, 3, Advance::TrackEnded), Some(1));
    assert_eq!(next_index(RepeatMode::One, 1, 3, Advance::Skipped), Some(2));
  }

  #[test]
  fn previous_restarts_late_in_a_track() {
    assert_eq!(
      previous(RepeatMode::Off, 2, 3, Duration::from_secs(10)),
      Previous::Restart
    );
    assert_eq!(previous(RepeatMode::Off, 2, 3, Duration::from_secs(1)), Previous::Go(1));
    assert_eq!(previous(RepeatMode::Off, 0, 3, Duration::ZERO), Previous::Restart);
    assert_eq!(previous(RepeatMode::All, 0, 3, Duration::ZERO), Previous::Go(2));
  }
}
