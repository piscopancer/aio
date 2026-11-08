//! What the use cases need from the outside world; infrastructure and interface implement these.

use std::path::Path;
use std::time::Duration;

use super::error::AppResult;
use super::views::*;
use crate::domain::download::{AudioQuality, Candidate, SongTags, VideoFacts};
use crate::domain::ids::{AlbumId, ArtistId, FolderId, PlaylistId, TrackId};
use crate::domain::library::{FolderPath, LibraryFolders};
use crate::domain::playback::{PlaybackContext, RepeatMode, Volume};
use crate::domain::playlist::{Playlist as PlaylistAggregate, PlaylistName};

pub trait FolderRepository: Send + Sync {
  fn load_folders(&self) -> AppResult<LibraryFolders>;
  /// Stores exactly this set of folders; tracks of removed folders go with them.
  fn save_folders(&self, folders: &LibraryFolders) -> AppResult<()>;
}

pub trait PlaylistRepository: Send + Sync {
  fn create_playlist(&self, name: &PlaylistName, cover: Option<&str>) -> AppResult<PlaylistId>;
  fn load_playlist(&self, id: PlaylistId) -> AppResult<PlaylistAggregate>;
  fn save_playlist(&self, playlist: &PlaylistAggregate) -> AppResult<()>;
  fn delete_playlist(&self, id: PlaylistId) -> AppResult<()>;
}

/// Images the user picks for the app's own artwork.
pub trait CoverImages: Send + Sync {
  /// Copies the image at `file` into the app's covers as a square, returning the stored copy's path.
  fn import(&self, file: &Path) -> AppResult<String>;
  /// The colour that covers most of the image at `file`, ignoring greys unless it has nothing else, as `#rrggbb`.
  fn dominant_color(&self, file: &Path) -> AppResult<String>;
}

/// Tag data of one audio file, as read during a scan.
pub struct ScannedTrack {
  pub path: String,
  pub title: String,
  pub artist: String,
  pub album: String,
  pub album_artist: String,
  pub year: Option<u32>,
  pub track_no: Option<u32>,
  pub disc_no: Option<u32>,
  pub duration_ms: u32,
  pub modified: i64,
  pub cover: Option<String>,
}

/// An audio file found on disk, before its tags are read.
pub struct AudioFile {
  pub path: String,
  pub modified: i64,
}

/// The stored index of tracks that scans keep in sync with the disk.
pub trait TrackIndex: Send + Sync {
  /// Path and modification time of every indexed file under a folder.
  fn indexed_files(&self, folder: &FolderPath) -> AppResult<Vec<AudioFile>>;
  fn index_tracks(&self, folder: &FolderPath, tracks: &[ScannedTrack]) -> AppResult<()>;
  fn drop_files(&self, paths: &[String]) -> AppResult<()>;
  /// Id of the indexed track stored at `path`.
  fn track_at(&self, path: &str) -> AppResult<TrackId>;
}

pub trait AudioFiles: Send + Sync {
  fn list(&self, folder: &FolderPath) -> Vec<AudioFile>;
  fn read(&self, file: &AudioFile) -> Option<ScannedTrack>;
  /// The audio file at `path`, if it exists.
  fn file(&self, path: &Path) -> Option<AudioFile>;
}

/// Read-only views of the library; they bypass the aggregates because they change nothing.
pub trait LibraryQueries: Send + Sync {
  fn folders(&self) -> AppResult<Vec<Folder>>;
  fn folder(&self, id: FolderId) -> AppResult<FolderDetail>;
  fn tracks(&self) -> AppResult<Vec<Track>>;
  fn tracks_by_ids(&self, ids: &[TrackId]) -> AppResult<Vec<Track>>;
  fn albums(&self) -> AppResult<Vec<Album>>;
  fn album(&self, id: AlbumId) -> AppResult<AlbumDetail>;
  fn artists(&self) -> AppResult<Vec<Artist>>;
  fn artist(&self, id: ArtistId) -> AppResult<ArtistDetail>;
  fn playlists(&self) -> AppResult<Vec<Playlist>>;
  fn playlist(&self, id: PlaylistId) -> AppResult<PlaylistDetail>;

  /// Tracks of a playback context in the order its view shows them.
  fn context_tracks(&self, context: PlaybackContext) -> AppResult<Vec<Track>> {
    match context {
      PlaybackContext::Songs => self.tracks(),
      PlaybackContext::Album { id } => Ok(self.album(id)?.tracks),
      PlaybackContext::Artist { id } => Ok(self.artist(id)?.tracks),
      PlaybackContext::Playlist { id } => Ok(self.playlist(id)?.tracks),
      PlaybackContext::Folder { id } => Ok(self.folder(id)?.tracks),
    }
  }
}

#[derive(Debug, Clone, Copy)]
pub struct PlaybackSettings {
  pub volume: Volume,
  pub shuffle: bool,
  pub repeat: RepeatMode,
}

pub trait SettingsStore: Send + Sync {
  fn playback_settings(&self) -> PlaybackSettings;
  fn save_playback_settings(&self, settings: PlaybackSettings) -> AppResult<()>;
}

/// The sound device; implementations live on the audio thread and need not be `Send`.
pub trait AudioOutput {
  /// Replaces whatever is loaded with this track, starting at its beginning.
  fn load(&mut self, track: &Track, volume: Volume, play: bool) -> AppResult<()>;
  fn unload(&mut self);
  fn play(&mut self);
  fn pause(&mut self);
  fn seek(&mut self, position: Duration) -> AppResult<()>;
  fn position(&self) -> Duration;
  /// True when a loaded track has played to its end.
  fn finished(&self) -> bool;
  fn set_volume(&mut self, volume: Volume);
}

/// Outgoing notifications to whoever displays the app.
pub trait Notifier: Send + Sync {
  fn player_changed(&self, state: &PlayerState);
  fn playback_progress(&self, position_ms: u32);
  fn playback_failed(&self, error: &super::error::AppError);
  fn library_changed(&self);
  fn scan_progress(&self, done: u32, total: u32);
  fn downloads_changed(&self, downloads: &[DownloadView]);
  fn tools_changed(&self, tools: &ToolsView);
}

/// Saved download options; a missing folder means the system Music folder.
#[derive(Debug, Clone, Default)]
pub struct DownloadSettings {
  pub folder: Option<String>,
  /// Unix seconds of the last update check of the download tools.
  pub tools_checked_at: Option<i64>,
}

pub trait DownloadSettingsStore: Send + Sync {
  fn download_settings(&self) -> DownloadSettings;
  fn save_download_settings(&self, settings: &DownloadSettings) -> AppResult<()>;
}

/// A video as the media site describes it.
pub struct VideoInfo {
  pub url: String,
  pub facts: VideoFacts,
  pub duration_ms: u32,
  pub thumbnail: Option<String>,
  /// Bitrate of the best AAC audio stream, the one the original quality keeps.
  pub original_kbps: Option<u32>,
  /// Release year the site states; the upload date is not taken, since re-uploads are common.
  pub year: Option<u16>,
}

/// A Spotify track as its public page describes it.
pub struct SpotifyTrack {
  pub title: String,
  pub artists: Vec<String>,
  pub album: Option<String>,
  pub duration_ms: u32,
  pub cover: Option<String>,
  pub year: Option<u16>,
}

/// One file to fetch: the video, the quality and the exact path the audio must end up at.
pub struct FetchJob<'a> {
  pub video_url: &'a str,
  pub quality: AudioQuality,
  pub target: &'a Path,
}

/// Steps of a fetch that the caller shows.
pub enum FetchProgress {
  Downloading(f32),
  Converting,
}

/// The streaming sites and the tools that read them.
pub trait MediaSource: Send + Sync {
  fn video(&self, url: &str) -> AppResult<VideoInfo>;
  /// Songs on YouTube Music matching the query, best first.
  fn search_songs(&self, query: &str) -> AppResult<Vec<Candidate>>;
  fn spotify_track(&self, id: &str) -> AppResult<SpotifyTrack>;
  fn fetch(&self, job: &FetchJob<'_>, progress: &mut dyn FnMut(FetchProgress)) -> AppResult<()>;
  /// Writes the tags and, when given, a square cover made from the image `cover` points to.
  fn write_tags(&self, path: &Path, tags: &SongTags, cover: Option<&CoverSource>) -> AppResult<()>;
}

/// The command line programs downloads run on.
pub trait DownloadTools: Send + Sync {
  fn versions(&self) -> ToolVersions;
  /// Downloads every tool that is not there yet, reporting each one's progress.
  fn install_missing(&self, progress: &mut dyn FnMut(ToolInstall)) -> AppResult<()>;
  /// Brings the site extractor up to date; sites change often enough to break older versions.
  fn update(&self) -> AppResult<()>;
}
