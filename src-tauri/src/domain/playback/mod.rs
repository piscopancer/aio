mod policy;
mod queue;

pub use policy::{Previous, resume_point};
pub use queue::{Advance, Following, Queue, QueueEntry, QueueKey, QueueSnapshot, Removal};

use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::ids::{AlbumId, ArtistId, FolderId, PlaylistId};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PlaybackError {
  #[error("volume must be between 0 and 100 percent")]
  VolumeOutOfRange,
  #[error("there is nothing to play")]
  NothingToPlay,
  #[error("crossfade must be between 0 and {max} seconds", max = Crossfade::MAX)]
  CrossfadeOutOfRange,
  #[error("this entry is no longer in the queue")]
  EntryNotInQueue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RepeatMode {
  #[default]
  Off,
  All,
  One,
}

impl RepeatMode {
  /// The mode the repeat button switches to: off, then the whole queue, then the one song, then off again.
  pub fn following(self) -> Self {
    match self {
      Self::Off => Self::All,
      Self::All => Self::One,
      Self::One => Self::Off,
    }
  }
}

/// Where the queue was started from, so a view can tell whether the playing track belongs to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlaybackContext {
  Songs,
  Album { id: AlbumId },
  Artist { id: ArtistId },
  Playlist { id: PlaylistId },
  Folder { id: FolderId },
}

/// Output level in percent, from silent (0) to full (100), as the user sets it on the slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Volume(u8);

impl Volume {
  pub const MAX: u8 = 100;
  pub const DEFAULT: Self = Self(70);
  /// No sound, what a muted player plays at.
  pub const SILENT: Self = Self(0);

  pub fn new(percent: u8) -> Result<Self, PlaybackError> {
    if percent <= Self::MAX {
      Ok(Self(percent))
    } else {
      Err(PlaybackError::VolumeOutOfRange)
    }
  }

  pub fn percent(self) -> u8 {
    self.0
  }

  /// Amplitude factor; squaring makes the lower half of the slider audible instead of almost silent.
  pub fn gain(self) -> f32 {
    let level = f32::from(self.0) / f32::from(Self::MAX);
    level * level
  }
}

/// How long the end of one song and the start of the next play over each other, in whole seconds; zero is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Crossfade(u8);

impl Crossfade {
  pub const MAX: u8 = 12;

  pub fn new(seconds: u8) -> Result<Self, PlaybackError> {
    if seconds <= Self::MAX {
      Ok(Self(seconds))
    } else {
      Err(PlaybackError::CrossfadeOutOfRange)
    }
  }

  pub fn seconds(self) -> u8 {
    self.0
  }

  /// How long `ending` and the song after it overlap: the set length, but no more than half of either, so a short song is never played over whole; `next` is unknown while a shuffled queue is still to be mixed.
  pub fn overlap(self, ending: Duration, next: Option<Duration>) -> Duration {
    let set = Duration::from_secs(self.0.into());
    let shortest = next.map_or(ending, |next| ending.min(next));
    set.min(shortest / 2)
  }
}

/// Whether a song that follows `ended` on its own goes on from it without a gap, as the songs of one album do, so neither is faded at the seam.
pub fn seamless(ended: Option<AlbumId>, next: Option<AlbumId>) -> bool {
  ended.is_some() && ended == next
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn volume_stays_in_range() {
    assert_eq!(Volume::new(101).unwrap_err(), PlaybackError::VolumeOutOfRange);
    assert_eq!(Volume::new(50).unwrap().gain(), 0.25);
    assert_eq!(Volume::new(0).unwrap().gain(), 0.0);
  }

  #[test]
  fn crossfade_takes_at_most_half_of_either_song() {
    assert_eq!(Crossfade::new(13).unwrap_err(), PlaybackError::CrossfadeOutOfRange);
    let crossfade = Crossfade::new(8).unwrap();
    let minutes = Duration::from_secs(180);
    assert_eq!(crossfade.overlap(minutes, Some(minutes)), Duration::from_secs(8));
    assert_eq!(
      crossfade.overlap(minutes, Some(Duration::from_secs(10))),
      Duration::from_secs(5)
    );
    assert_eq!(crossfade.overlap(Duration::from_secs(6), None), Duration::from_secs(3));
    assert_eq!(Crossfade::default().overlap(minutes, None), Duration::ZERO);
  }

  #[test]
  fn only_songs_of_one_album_join_without_a_seam() {
    assert!(seamless(Some(AlbumId(1)), Some(AlbumId(1))));
    assert!(!seamless(Some(AlbumId(1)), Some(AlbumId(2))));
    assert!(!seamless(None, None));
  }
}
