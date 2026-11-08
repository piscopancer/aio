//! Read models handed to the interface; they are plain data and carry no rules.

use serde::Serialize;
use specta::Type;

use crate::application::error::AppError;
use crate::domain::download::{AudioQuality, Service};
use crate::domain::ids::{AlbumId, ArtistId, DownloadId, FolderId, PlaylistId, TrackId};
use crate::domain::playback::{PlaybackContext, QueueKey, RepeatMode};

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
  pub id: FolderId,
  pub path: String,
  pub track_count: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Track {
  pub id: TrackId,
  pub path: String,
  pub title: String,
  /// The credited artists in order, the main one first; never empty.
  pub artists: Vec<ArtistName>,
  pub album_id: AlbumId,
  pub album: String,
  pub cover: Option<String>,
  pub track_no: Option<u32>,
  pub disc_no: Option<u32>,
  pub duration_ms: u32,
  /// The file left the library; only playlists still list the track, until the file is back.
  pub missing: bool,
}

/// An artist as a song credits it, enough to link to the artist's page.
#[derive(Debug, Clone, Serialize, Type)]
pub struct ArtistName {
  pub id: ArtistId,
  pub name: String,
}

/// Tags of one song file as the edit dialog shows them, with facts about the file next to them.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackTags {
  pub path: String,
  pub title: String,
  /// The credited artists in order; empty when the file names none.
  pub artists: Vec<String>,
  /// Set only when the file names an album artist of its own, as compilations do.
  pub album_artist: Option<String>,
  pub album: Option<String>,
  pub year: Option<u16>,
  pub track_no: Option<u32>,
  pub disc_no: Option<u32>,
  pub genre: Option<String>,
  /// The file type by its extension, such as `MP3`.
  pub format: String,
  pub bitrate_kbps: Option<u32>,
  pub duration_ms: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Album {
  pub id: AlbumId,
  pub title: String,
  pub artist_id: ArtistId,
  pub artist: String,
  pub year: Option<u32>,
  pub cover: Option<String>,
  pub track_count: u32,
  pub duration_ms: u32,
  /// Place among the pinned of its list, the first being 0; none when it is not pinned.
  pub pin: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
  pub id: ArtistId,
  pub name: String,
  pub cover: Option<String>,
  pub album_count: u32,
  pub track_count: u32,
  /// Place among the pinned of its list, the first being 0; none when it is not pinned.
  pub pin: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
  pub id: PlaylistId,
  pub name: String,
  /// The picked image, or else the art of the first song that has any.
  pub cover: Option<String>,
  /// The picked image alone, which the edit dialog changes.
  pub own_cover: Option<String>,
  pub track_count: u32,
  pub duration_ms: u32,
  /// Unix seconds when the playlist was made.
  pub created_at: u32,
  /// Unix seconds when playback last started from the playlist, none when it never did.
  pub played_at: Option<u32>,
  /// Place among the pinned of its list, the first being 0; none when it is not pinned.
  pub pin: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct AlbumDetail {
  pub album: Album,
  pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistDetail {
  pub artist: Artist,
  pub albums: Vec<Album>,
  /// Albums of other artists with songs that credit this one.
  pub appears_on: Vec<Album>,
  /// Every song that credits the artist, theirs or a guest spot.
  pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct PlaylistDetail {
  pub playlist: Playlist,
  pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct FolderDetail {
  pub folder: Folder,
  pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct QueueItem {
  pub key: QueueKey,
  pub track: Track,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
  pub context: Option<PlaybackContext>,
  pub queue: Vec<QueueItem>,
  pub index: Option<u32>,
  pub is_playing: bool,
  pub position_ms: u32,
  /// Percent from 0 to 100; it keeps the level while muted.
  pub volume: u8,
  pub muted: bool,
  pub shuffle: bool,
  pub repeat: RepeatMode,
}

/// One quality the preview offers, with the size it would roughly take.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QualityOption {
  pub quality: AudioQuality,
  pub kbps: u32,
  pub estimated_bytes: u32,
}

/// What a pasted link resolves to, with tags guessed for the user to correct.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadPreview {
  pub service: Service,
  /// The video the audio comes from; for Spotify links, the best match on YouTube Music.
  pub video_url: String,
  pub cover_url: Option<String>,
  pub duration_ms: u32,
  pub title: String,
  pub artists: Vec<String>,
  pub album: Option<String>,
  pub year: Option<u16>,
  pub qualities: Vec<QualityOption>,
}

/// How a download folder relates to the library.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FolderCheck {
  /// Inside the library folder `library`.
  InLibrary { path: String, library: String },
  /// Outside the library; it is added on download.
  WillBeAdded { path: String },
  /// It contains the library folder `existing`, so it cannot be added.
  Overlaps { path: String, existing: String },
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DownloadStage {
  Queued,
  Downloading { percent: u8 },
  Converting,
  Tagging,
  Done { track_id: TrackId },
  Failed { error: AppError },
}

/// Where the artwork of a download comes from: the site's own image, or a file the user picked instead.
#[derive(Debug, Clone, Serialize, serde::Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CoverSource {
  Web {
    url: String,
  },
  /// A square copy the user picked, stored by `import_cover`.
  File {
    path: String,
  },
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadView {
  pub id: DownloadId,
  pub title: String,
  pub artist: String,
  pub cover: Option<CoverSource>,
  pub folder: String,
  pub stage: DownloadStage,
}

/// The command line programs downloads need, by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Tool {
  YtDlp,
  Ffmpeg,
  Deno,
}

/// Installed version of each tool, missing ones as null.
#[derive(Debug, Clone, Default, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolVersions {
  pub yt_dlp: Option<String>,
  pub ffmpeg: Option<String>,
  pub deno: Option<String>,
}

impl ToolVersions {
  pub fn complete(&self) -> bool {
    self.yt_dlp.is_some() && self.ffmpeg.is_some() && self.deno.is_some()
  }
}

/// A tool being fetched; `total_bytes` is null when the server does not say.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolInstall {
  pub tool: Tool,
  pub done_bytes: u32,
  pub total_bytes: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolsView {
  pub versions: ToolVersions,
  pub installing: Option<ToolInstall>,
  pub updating: bool,
  /// Unix seconds of the last update check.
  pub checked_at: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSettingsView {
  /// The folder downloads go to unless the download picks another one.
  pub folder: String,
}

/// Where shown lyrics came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LyricsOrigin {
  /// The song file had them before Aio looked.
  File,
  Lrclib {
    id: u32,
  },
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LyricWord {
  pub start_ms: u32,
  pub text: String,
}

/// One sung line; `words` is empty unless every word is timed, and `text` is empty for an instrumental gap.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
  pub start_ms: u32,
  pub text: String,
  pub words: Vec<LyricWord>,
}

/// The lyrics panel's content for one song.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LyricsView {
  Synced {
    lines: Vec<LyricLine>,
    origin: LyricsOrigin,
  },
  Plain {
    text: String,
    origin: LyricsOrigin,
  },
  Instrumental,
  Missing,
}
