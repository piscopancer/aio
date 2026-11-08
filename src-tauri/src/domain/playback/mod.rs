mod policy;
mod queue;

pub use policy::Previous;
pub use queue::{Advance, Queue, QueueEntry, QueueKey, Removal};

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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn volume_stays_in_range() {
    assert_eq!(Volume::new(101).unwrap_err(), PlaybackError::VolumeOutOfRange);
    assert_eq!(Volume::new(50).unwrap().gain(), 0.25);
    assert_eq!(Volume::new(0).unwrap().gain(), 0.0);
  }
}
