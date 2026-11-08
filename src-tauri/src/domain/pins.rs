//! Pinned playlists, albums and artists: each kind keeps its own order, set by hand, which leads its list.

use serde::{Deserialize, Serialize};
use specta::Type;

use super::ids::{AlbumId, ArtistId, PlaylistId};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PinError {
  #[error("this is not pinned")]
  NotPinned,
}

/// Something that can be pinned to the top of its list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PinTarget {
  Playlist { id: PlaylistId },
  Album { id: AlbumId },
  Artist { id: ArtistId },
}

/// The lists that have pins of their own: the sidebar's playlists, the Albums tab and the Artists tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinKind {
  Playlist,
  Album,
  Artist,
}

impl PinTarget {
  pub fn kind(self) -> PinKind {
    match self {
      Self::Playlist { .. } => PinKind::Playlist,
      Self::Album { .. } => PinKind::Album,
      Self::Artist { .. } => PinKind::Artist,
    }
  }

  /// The id within its kind.
  pub fn raw(self) -> u32 {
    match self {
      Self::Playlist { id } => id.0,
      Self::Album { id } => id.0,
      Self::Artist { id } => id.0,
    }
  }
}

/// The pinned items of one kind in their order, each once.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pins(Vec<u32>);

impl Pins {
  pub fn new(ids: impl IntoIterator<Item = u32>) -> Self {
    let mut pins = Self::default();
    for id in ids {
      if !pins.0.contains(&id) {
        pins.0.push(id);
      }
    }
    pins
  }

  pub fn ids(&self) -> &[u32] {
    &self.0
  }

  /// Puts the item first among the pinned; one pinned already keeps its place.
  pub fn pin(&mut self, id: u32) {
    if !self.0.contains(&id) {
      self.0.insert(0, id);
    }
  }

  pub fn unpin(&mut self, id: u32) {
    self.0.retain(|pinned| *pinned != id);
  }

  /// Moves a pinned item to place `to` among the pinned, or to the end when `to` is past it.
  pub fn move_to(&mut self, id: u32, to: usize) -> Result<(), PinError> {
    let from = self
      .0
      .iter()
      .position(|pinned| *pinned == id)
      .ok_or(PinError::NotPinned)?;
    let id = self.0.remove(from);
    self.0.insert(to.min(self.0.len()), id);
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn new_pins_go_first_and_stay_once() {
    let mut pins = Pins::new([1, 2, 1]);
    assert_eq!(pins.ids(), [1, 2]);
    pins.pin(3);
    pins.pin(2);
    assert_eq!(pins.ids(), [3, 1, 2]);
    pins.unpin(1);
    assert_eq!(pins.ids(), [3, 2]);
  }

  #[test]
  fn pins_move_by_hand() {
    let mut pins = Pins::new([1, 2, 3]);
    pins.move_to(1, 2).unwrap();
    assert_eq!(pins.ids(), [2, 3, 1]);
    pins.move_to(1, 99).unwrap();
    assert_eq!(pins.ids(), [2, 3, 1]);
    pins.move_to(3, 0).unwrap();
    assert_eq!(pins.ids(), [3, 2, 1]);
    assert_eq!(pins.move_to(7, 0), Err(PinError::NotPinned));
  }
}
