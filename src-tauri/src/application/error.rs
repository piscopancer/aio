use serde::Serialize;
use specta::Type;

use crate::domain::download::DownloadError;
use crate::domain::ids::{AlbumId, ArtistId, FolderId, PlaylistId, TrackId};
use crate::domain::library::LibraryError;
use crate::domain::pins::PinError;
use crate::domain::playback::PlaybackError;
use crate::domain::playlist::PlaylistError;
use crate::domain::tags::TagError;

/// Every failure a use case can report; domain rejections keep their own typed reasons.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(tag = "type", content = "reason", rename_all = "camelCase")]
pub enum AppError {
  #[error(transparent)]
  Library(#[from] LibraryError),
  #[error(transparent)]
  Playlist(#[from] PlaylistError),
  #[error(transparent)]
  Playback(#[from] PlaybackError),
  #[error(transparent)]
  Download(#[from] DownloadError),
  #[error(transparent)]
  Tags(#[from] TagError),
  #[error(transparent)]
  Pin(#[from] PinError),
  /// A record that was looked up does not exist, most often because it was removed meanwhile.
  #[error("{0} not found")]
  NotFound(Missing),
  #[error("storage failed: {0}")]
  Storage(String),
  #[error("audio failed: {0}")]
  Audio(String),
  /// A picked image file cannot be read or decoded.
  #[error("this image cannot be used: {0}")]
  Image(String),
  /// A song file cannot be read or rewritten.
  #[error("the song file cannot be changed: {0}")]
  SongFile(String),
  /// A download tool, the network or the media site failed.
  #[error("{0}")]
  Fetch(String),
}

pub type AppResult<T> = Result<T, AppError>;

/// The record a lookup did not find, with the id or path it was looked up by.
#[derive(Debug, Clone, thiserror::Error, Serialize, Type)]
#[serde(tag = "kind", content = "key", rename_all = "camelCase")]
pub enum Missing {
  #[error("song {}", .0.0)]
  Track(TrackId),
  #[error("song file {0}")]
  TrackFile(String),
  #[error("album {}", .0.0)]
  Album(AlbumId),
  #[error("artist {}", .0.0)]
  Artist(ArtistId),
  #[error("playlist {}", .0.0)]
  Playlist(PlaylistId),
  #[error("folder {}", .0.0)]
  Folder(FolderId),
  #[error("library folder {0}")]
  FolderPath(String),
  /// A storage lookup with no better description, which should not happen.
  #[error("stored record")]
  Record,
}
