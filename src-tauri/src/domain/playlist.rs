use serde::Serialize;
use specta::Type;

use super::ids::{PlaylistId, TrackId};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PlaylistError {
  #[error("a playlist needs a name")]
  NameEmpty,
  #[error("a playlist name can be at most {max} characters long")]
  NameTooLong { max: u32 },
  #[error("this track is not in the playlist")]
  TrackNotInPlaylist,
}

/// Trimmed, non-empty playlist name; names may repeat across playlists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistName(String);

impl PlaylistName {
  pub const MAX_CHARS: u32 = 100;

  pub fn new(name: &str) -> Result<Self, PlaylistError> {
    let name = name.trim();
    if name.is_empty() {
      return Err(PlaylistError::NameEmpty);
    }
    if name.chars().count() > Self::MAX_CHARS as usize {
      return Err(PlaylistError::NameTooLong { max: Self::MAX_CHARS });
    }
    Ok(Self(name.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// How many of the requested tracks a playlist took and how many it already had.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct AddOutcome {
  pub added: u32,
  pub skipped: u32,
}

/// Aggregate root of an ordered playlist; each track appears in it at most once.
#[derive(Debug, Clone)]
pub struct Playlist {
  id: PlaylistId,
  name: PlaylistName,
  /// Stored image the user picked; without one the art of the playlist's songs stands in.
  cover: Option<String>,
  tracks: Vec<TrackId>,
}

impl Playlist {
  /// Rebuilds a stored playlist; storage only ever holds playlists that passed these rules.
  pub fn restore(id: PlaylistId, name: PlaylistName, cover: Option<String>, tracks: Vec<TrackId>) -> Self {
    let mut playlist = Self {
      id,
      name,
      cover,
      tracks: Vec::with_capacity(tracks.len()),
    };
    playlist.add(&tracks);
    playlist
  }

  pub fn id(&self) -> PlaylistId {
    self.id
  }

  pub fn name(&self) -> &PlaylistName {
    &self.name
  }

  pub fn tracks(&self) -> &[TrackId] {
    &self.tracks
  }

  pub fn cover(&self) -> Option<&str> {
    self.cover.as_deref()
  }

  pub fn change_cover(&mut self, cover: Option<String>) {
    self.cover = cover;
  }

  pub fn rename(&mut self, name: PlaylistName) {
    self.name = name;
  }

  /// Appends the tracks in order, skipping ones already in the playlist or repeated in the request.
  pub fn add(&mut self, tracks: &[TrackId]) -> AddOutcome {
    let before = self.tracks.len();
    for track in tracks {
      if !self.tracks.contains(track) {
        self.tracks.push(*track);
      }
    }
    let added = self.tracks.len() - before;
    AddOutcome {
      added: count(added),
      skipped: count(tracks.len() - added),
    }
  }

  pub fn remove(&mut self, tracks: &[TrackId]) -> Result<(), PlaylistError> {
    if tracks.iter().any(|track| !self.tracks.contains(track)) {
      return Err(PlaylistError::TrackNotInPlaylist);
    }
    self.tracks.retain(|track| !tracks.contains(track));
    Ok(())
  }

  /// Moves a track to position `to` of the resulting order, clamped to the end.
  pub fn move_track(&mut self, track: TrackId, to: usize) -> Result<(), PlaylistError> {
    let from = self
      .tracks
      .iter()
      .position(|t| *t == track)
      .ok_or(PlaylistError::TrackNotInPlaylist)?;
    let moved = self.tracks.remove(from);
    self.tracks.insert(to.min(self.tracks.len()), moved);
    Ok(())
  }
}

fn count(n: usize) -> u32 {
  u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn tracks(ids: &[u32]) -> Vec<TrackId> {
    ids.iter().copied().map(TrackId).collect()
  }

  fn playlist(ids: &[u32]) -> Playlist {
    let tracks = tracks(ids);
    Playlist::restore(PlaylistId(1), PlaylistName::new("Mix").unwrap(), None, tracks)
  }

  fn ids(playlist: &Playlist) -> Vec<u32> {
    playlist.tracks().iter().map(|t| t.0).collect()
  }

  #[test]
  fn names_are_trimmed_and_required() {
    assert_eq!(PlaylistName::new("  Focus ").unwrap().as_str(), "Focus");
    assert_eq!(PlaylistName::new("   ").unwrap_err(), PlaylistError::NameEmpty);
    assert!(matches!(
      PlaylistName::new(&"a".repeat(101)),
      Err(PlaylistError::NameTooLong { .. })
    ));
  }

  #[test]
  fn adding_skips_tracks_already_present() {
    let mut list = playlist(&[1, 2]);
    let outcome = list.add(&tracks(&[2, 3, 3, 4]));
    assert_eq!(outcome, AddOutcome { added: 2, skipped: 2 });
    assert_eq!(ids(&list), [1, 2, 3, 4]);
  }

  #[test]
  fn removing_a_missing_track_changes_nothing() {
    let mut list = playlist(&[1, 2]);
    assert_eq!(
      list.remove(&tracks(&[2, 9])).unwrap_err(),
      PlaylistError::TrackNotInPlaylist
    );
    assert_eq!(ids(&list), [1, 2]);
  }

  #[test]
  fn moves_tracks_within_bounds() {
    let mut list = playlist(&[1, 2, 3]);
    list.move_track(TrackId(1), 99).unwrap();
    assert_eq!(ids(&list), [2, 3, 1]);
    list.move_track(TrackId(1), 0).unwrap();
    assert_eq!(ids(&list), [1, 2, 3]);
    assert_eq!(
      list.move_track(TrackId(7), 0).unwrap_err(),
      PlaylistError::TrackNotInPlaylist
    );
  }
}
