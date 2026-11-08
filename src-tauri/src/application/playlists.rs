use std::sync::Arc;

use super::downloads::now;
use super::error::AppResult;
use super::ports::PlaylistRepository;
use crate::domain::ids::{PlaylistId, TrackId};
use crate::domain::playlist::{AddOutcome, PlaylistName};

/// Playlist changes, each one a load → domain action → save round trip on the playlist aggregate.
pub struct PlaylistService {
  playlists: Arc<dyn PlaylistRepository>,
}

impl PlaylistService {
  pub fn new(playlists: Arc<dyn PlaylistRepository>) -> Self {
    Self { playlists }
  }

  /// `cover` is a path `CoverService::import` returned, or none to let the songs' art stand in.
  pub fn create(&self, name: &str, cover: Option<&str>) -> AppResult<PlaylistId> {
    self.playlists.create_playlist(&PlaylistName::new(name)?, cover)
  }

  /// Renames the playlist and sets its cover, which takes the same values as in `create`.
  pub fn edit(&self, id: PlaylistId, name: &str, cover: Option<String>) -> AppResult<()> {
    let name = PlaylistName::new(name)?;
    let mut playlist = self.playlists.load_playlist(id)?;
    playlist.rename(name);
    playlist.change_cover(cover);
    self.playlists.save_playlist(&playlist)
  }

  /// Remembers that playback just started from the playlist, which orders it among the recently played.
  pub fn played(&self, id: PlaylistId) -> AppResult<()> {
    self.playlists.mark_playlist_played(id, now())
  }

  pub fn delete(&self, id: PlaylistId) -> AppResult<()> {
    self.playlists.delete_playlist(id)
  }

  pub fn add_tracks(&self, id: PlaylistId, tracks: &[TrackId]) -> AppResult<AddOutcome> {
    let mut playlist = self.playlists.load_playlist(id)?;
    let outcome = playlist.add(tracks);
    self.playlists.save_playlist(&playlist)?;
    Ok(outcome)
  }

  pub fn remove_tracks(&self, id: PlaylistId, tracks: &[TrackId]) -> AppResult<()> {
    let mut playlist = self.playlists.load_playlist(id)?;
    playlist.remove(tracks)?;
    self.playlists.save_playlist(&playlist)
  }

  pub fn move_track(&self, id: PlaylistId, track: TrackId, to: u32) -> AppResult<()> {
    let mut playlist = self.playlists.load_playlist(id)?;
    playlist.move_track(track, to as usize)?;
    self.playlists.save_playlist(&playlist)
  }
}
