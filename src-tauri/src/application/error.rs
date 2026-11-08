use serde::Serialize;
use specta::Type;

use crate::domain::download::DownloadError;
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
  #[error("not found")]
  NotFound,
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
