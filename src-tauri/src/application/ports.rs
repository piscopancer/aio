//! What the use cases need from the outside world; infrastructure and interface implement these.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde::Deserialize;
use specta::Type;

use super::downloads::DownloadRequest;
use super::error::AppResult;
use super::views::*;
use crate::domain::artists::Artists;
use crate::domain::download::{AudioQuality, Candidate, VideoFacts};
use crate::domain::ids::{AlbumId, ArtistId, DownloadId, FolderId, PlayId, PlaylistId, TrackId};
use crate::domain::library::{FolderPath, LibraryFolders};
use crate::domain::pins::{PinKind, Pins};
use crate::domain::playback::{Crossfade, Listen, PlaybackContext, QueueSnapshot, RepeatMode, Volume};
use crate::domain::playlist::{Playlist as PlaylistAggregate, PlaylistName};
use crate::domain::tags::{ReleaseType, SongTags};

pub trait FolderRepository: Send + Sync {
  fn load_folders(&self) -> AppResult<LibraryFolders>;
  /// Stores exactly this set of folders; tracks of removed folders move to the folder now holding them, or go with them.
  fn save_folders(&self, folders: &LibraryFolders) -> AppResult<()>;
  fn set_folder_hidden(&self, folder: &FolderPath, hidden: bool) -> AppResult<()>;
}

pub trait PlaylistRepository: Send + Sync {
  fn create_playlist(&self, name: &PlaylistName, cover: Option<&str>) -> AppResult<PlaylistId>;
  fn load_playlist(&self, id: PlaylistId) -> AppResult<PlaylistAggregate>;
  fn save_playlist(&self, playlist: &PlaylistAggregate) -> AppResult<()>;
  fn delete_playlist(&self, id: PlaylistId) -> AppResult<()>;
  /// Notes that playback started from the playlist at `at`, in Unix seconds.
  fn mark_playlist_played(&self, id: PlaylistId, at: i64) -> AppResult<()>;
}

/// Images the user picks for the app's own artwork.
/// Where the pins of each kind are kept.
pub trait PinStore: Send + Sync {
  fn load_pins(&self, kind: PinKind) -> AppResult<Pins>;
  /// Saves the pins of `kind` in their order; ids no longer in the library are skipped.
  fn save_pins(&self, kind: PinKind, pins: &Pins) -> AppResult<()>;
}

pub trait CoverImages: Send + Sync {
  /// Copies the image at `file` into the app's covers as a square, returning the stored copy's path.
  fn import(&self, file: &Path) -> AppResult<String>;
  /// The colour that covers most of the image at `file`, ignoring greys unless it has nothing else, as `#rrggbb`.
  /// Stores a square copy of an image given as the bytes of its file, returning the stored copy's path.
  fn import_bytes(&self, data: &[u8]) -> AppResult<String>;
  fn dominant_color(&self, file: &Path) -> AppResult<String>;
  /// A few colours of the image at `file` that sit well together and carry white text, strongest first, as `#rrggbb`.
  fn palette(&self, file: &Path) -> AppResult<Vec<String>>;
}

/// Tag data of one audio file, as read during a scan.
pub struct ScannedTrack {
  pub path: String,
  pub title: String,
  /// The credited artists; a song whose file names none is credited to nobody.
  pub artists: Option<Artists>,
  /// The album the file names; a song without one belongs to no album.
  pub album: Option<String>,
  /// The album artists the file names, or else its main artist; none when the file names no artist at all.
  pub album_artists: Option<Artists>,
  pub year: Option<u32>,
  pub release_type: Option<ReleaseType>,
  pub track_no: Option<u32>,
  pub disc_no: Option<u32>,
  pub genres: Vec<String>,
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
  /// Re-indexes files already in the library after their tags were rewritten; their albums take the cover and year
  /// the files now carry, even over ones they had.
  fn retag_tracks(&self, tracks: &[ScannedTrack]) -> AppResult<()>;
}

/// What happens to the front cover of a song file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CoverChange {
  Keep,
  /// The image file at `path` becomes the cover.
  Set {
    path: String,
  },
  Remove,
}

/// What the songs of one album share, written into the others when one of them is edited.
pub struct AlbumShare<'a> {
  pub year: Option<u16>,
  pub release_type: Option<ReleaseType>,
  /// The album artists, when the album changes hands; none leaves those of each song as they are, and an empty list
  /// gives the album to nobody.
  pub owners: Option<&'a [String]>,
  pub cover: &'a CoverChange,
}

/// Reading and rewriting the tags of single song files.
pub trait SongFiles: Send + Sync {
  fn tags(&self, path: &Path) -> AppResult<TrackTags>;
  /// Writes every song tag and changes the cover.
  fn write_song(&self, path: &Path, tags: &SongTags, cover: &CoverChange) -> AppResult<()>;
  /// Writes only what belongs to the album, leaving the song's own tags alone.
  fn write_album_art(&self, path: &Path, share: &AlbumShare) -> AppResult<()>;
  /// The lyrics the file holds in its tags, or else in a `.lrc` file beside it; blank text counts as none.
  fn lyrics(&self, path: &Path) -> AppResult<Option<String>>;
  /// Writes LRC or plain text into the file's lyrics tag.
  fn write_lyrics(&self, path: &Path, text: &str) -> AppResult<()>;
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
  /// The genres the library's songs name, the most common first.
  fn genres(&self) -> AppResult<Vec<Genre>>;
  fn artist(&self, id: ArtistId) -> AppResult<ArtistDetail>;
  fn playlists(&self) -> AppResult<Vec<Playlist>>;
  fn playlist(&self, id: PlaylistId) -> AppResult<PlaylistDetail>;
  /// The playlists that already hold the track.
  fn playlists_with_track(&self, track: TrackId) -> AppResult<Vec<PlaylistId>>;
  /// When the played songs and playlists were last played, the latest first.
  fn scores(&self) -> AppResult<Scores>;
  /// The `limit` songs whose files changed last, as a download or a copy into a folder does, the newest first.
  fn newest_tracks(&self, limit: u32) -> AppResult<Vec<Track>>;

  /// Tracks of a playback context in the order its view shows them.
  fn context_tracks(&self, context: PlaybackContext) -> AppResult<Vec<Track>> {
    match context {
      PlaybackContext::Songs => self.tracks(),
      PlaybackContext::Album { id } => Ok(self.album(id)?.tracks),
      PlaybackContext::Artist { id } => Ok(self.artist(id)?.tracks),
      PlaybackContext::Playlist { id } => Ok(
        self
          .playlist(id)?
          .tracks
          .into_iter()
          .filter(|t| t.presence == Presence::Present)
          .collect(),
      ),
      PlaybackContext::Folder { id } => Ok(self.folder(id)?.tracks),
      PlaybackContext::Recent => super::home::recent_songs(self),
      PlaybackContext::Added => super::home::added_songs(self),
    }
  }
}

/// When the played songs and the playlists played from were last played, as Unix seconds of the start of their latest
/// play that counted, the latest first; a playlist counts only what was played from it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scores {
  pub tracks: Vec<(TrackId, u32)>,
  pub playlists: Vec<(PlaylistId, u32)>,
}

/// The play history, which the player adds to as it plays.
pub trait PlayHistory: Send + Sync {
  /// Keeps a listen the player is in the middle of, giving it its id.
  fn add_listen(&self, listen: &Listen) -> AppResult<PlayId>;
  /// Stores how long a kept listen was heard by now and whether that counts as a play.
  fn update_listen(&self, id: PlayId, listen: &Listen) -> AppResult<()>;
}

#[derive(Debug, Clone, Copy)]
pub struct PlaybackSettings {
  pub volume: Volume,
  pub shuffle: bool,
  pub repeat: RepeatMode,
  /// Whether songs fade in and out as they start, stop, pause and resume.
  pub fades: bool,
  pub crossfade: Crossfade,
  /// Whether Discord shows the playing song on the user's profile.
  pub discord_status: bool,
}

/// What the player was doing when the app last ran: its queue, each song named by its id, and the position in the
/// current song.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSession {
  pub queue: QueueSnapshot<TrackId>,
  pub position_ms: u32,
}

pub trait SettingsStore: Send + Sync {
  fn playback_settings(&self) -> PlaybackSettings;
  fn save_playback_settings(&self, settings: PlaybackSettings) -> AppResult<()>;
  /// The session saved last, if there is one that can still be read.
  fn player_session(&self) -> Option<PlayerSession>;
  fn save_player_session(&self, session: &PlayerSession) -> AppResult<()>;
  /// Moves the saved session to another position in its song, leaving its queue as it is.
  fn save_player_position(&self, position_ms: u32) -> AppResult<()>;
}

/// The sound device; implementations live on the audio thread and need not be `Send`. A `fade` of zero cuts at once.
pub trait AudioOutput {
  /// Replaces whatever is loaded with this track, starting at its beginning; the track replaced fades out over `fade`
  /// while this one fades in over it, at once when `play`.
  fn load(&mut self, track: &Track, volume: Volume, play: bool, fade: Duration) -> AppResult<()>;
  /// Fades the loaded track out and lets it go.
  fn unload(&mut self, fade: Duration);
  /// Cuts off the tracks still fading out, letting go of their files.
  fn cut_leaving(&mut self);
  fn play(&mut self, fade: Duration);
  /// Fades the loaded track out and holds it where it then is, along with any still fading out.
  fn pause(&mut self, fade: Duration);
  fn seek(&mut self, position: Duration) -> AppResult<()>;
  fn position(&self) -> Duration;
  /// True when a loaded track has played to its end.
  fn finished(&self) -> bool;
  fn set_volume(&mut self, volume: Volume);
  /// Fades out the last `length` of the loaded track as it plays to its end; zero plays it to the end at full.
  fn fade_tail(&mut self, length: Duration);
}

/// Outgoing notifications to whoever displays the app.
pub trait Notifier: Send + Sync {
  fn player_changed(&self, state: &PlayerState);
  fn playback_progress(&self, position_ms: u32);
  fn playback_failed(&self, error: &super::error::AppError);
  fn library_changed(&self);
  /// The play history gained a play or more time heard, so the home page is stale.
  fn history_changed(&self);
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

pub trait WindowSettingsStore: Send + Sync {
  fn window_settings(&self) -> WindowSettings;
  fn save_window_settings(&self, settings: WindowSettings) -> AppResult<()>;
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
  /// Set when the user cancels; the fetch then stops the tool, removes what it wrote and fails.
  pub cancelled: &'a AtomicBool,
}

/// How a download ended: with its song in the library, or with the reason it failed in words.
pub enum DownloadOutcome {
  Done(TrackId),
  Failed(String),
}

/// A download as it is kept across launches.
pub struct SavedDownload {
  pub id: DownloadId,
  pub request: DownloadRequest,
  /// Where the song goes, resolved when it started, so a later change of the download folder leaves it alone.
  pub folder: PathBuf,
  /// The file the song is written to, picked once it first ran.
  pub file_name: Option<String>,
  /// None while it has not ended, which on launch means quitting cut it short.
  pub outcome: Option<DownloadOutcome>,
}

/// The download settings, and the downloads the list shows, kept until the user dismisses or cancels them.
pub trait DownloadStore: DownloadSettingsStore {
  /// Every kept download, oldest first.
  fn saved_downloads(&self) -> AppResult<Vec<SavedDownload>>;
  /// Keeps a new running download and gives it its id.
  fn add_download(&self, request: &DownloadRequest, folder: &Path) -> AppResult<DownloadId>;
  fn set_download_file(&self, id: DownloadId, file_name: &str) -> AppResult<()>;
  fn end_download(&self, id: DownloadId, outcome: &DownloadOutcome) -> AppResult<()>;
  fn remove_download(&self, id: DownloadId) -> AppResult<()>;
}

/// Steps of a fetch that the caller shows.
pub enum FetchProgress {
  /// The share of the file received so far, and the size of the whole file in bytes, which some sites only estimate.
  Downloading {
    fraction: f32,
    total_bytes: u32,
  },
  Converting,
}

/// The streaming sites and the tools that read them.
pub trait MediaSource: Send + Sync {
  fn video(&self, url: &str) -> AppResult<VideoInfo>;
  /// Songs on YouTube Music matching the query, best first.
  fn search_songs(&self, query: &str) -> AppResult<Vec<Candidate>>;
  fn spotify_track(&self, id: &str) -> AppResult<SpotifyTrack>;
  /// A direct address of the audio of the video at `url`, which plays for a few hours before it expires.
  fn audio_stream(&self, url: &str) -> AppResult<String>;
  fn fetch(&self, job: &FetchJob<'_>, progress: &mut dyn FnMut(FetchProgress)) -> AppResult<()>;
  /// Writes the tags and, when given, a square cover made from the image `cover` points to.
  fn write_tags(&self, path: &Path, tags: &SongTags, cover: Option<&CoverSource>) -> AppResult<()>;
}

/// A catalogue that finds songs to download by their name.
pub trait SongFinder: Send + Sync {
  fn find_songs(&self, query: &str) -> AppResult<SongFinds>;
}

/// The command line programs downloads run on.
pub trait DownloadTools: Send + Sync {
  fn versions(&self) -> ToolVersions;
  /// Downloads every tool that is not there yet, reporting each one's progress.
  fn install_missing(&self, progress: &mut dyn FnMut(ToolInstall)) -> AppResult<()>;
  /// Brings the site extractor up to date; sites change often enough to break older versions.
  fn update(&self) -> AppResult<()>;
}

/// What a song is looked up by on a lyrics site.
pub struct LyricsQuery<'a> {
  pub title: &'a str,
  pub artist: Option<&'a str>,
  pub album: Option<&'a str>,
  pub duration_ms: u32,
}

/// What a lookup of lyrics ended with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LyricsFind {
  Text { text: String, origin: LyricsOrigin },
  Instrumental,
  Missing,
}

/// Lyrics a site has for a song, and the length of the recording they are timed for.
pub struct OnlineLyrics {
  pub find: LyricsFind,
  pub length_ms: u32,
}

pub trait LyricsFinder: Send + Sync {
  fn find(&self, song: &LyricsQuery<'_>) -> AppResult<Option<OnlineLyrics>>;
}

/// What Aio remembers of a song's lyrics; found text is kept only until the song file holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LyricsState {
  /// Found text waiting to be written into the song file.
  Unwritten {
    text: String,
    origin: LyricsOrigin,
  },
  /// Text the song file holds now, remembered only for where it came from.
  Written {
    origin: LyricsOrigin,
  },
  Instrumental,
  Missing,
}

impl From<LyricsFind> for LyricsState {
  /// A new find waits to be written into the file.
  fn from(find: LyricsFind) -> Self {
    match find {
      LyricsFind::Text { text, origin } => Self::Unwritten { text, origin },
      LyricsFind::Instrumental => Self::Instrumental,
      LyricsFind::Missing => Self::Missing,
    }
  }
}

/// What Aio knows about a song's lyrics beyond its file: their state, the length they were timed for, and when they
/// were last looked up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsRecord {
  pub state: LyricsState,
  pub length_ms: u32,
  /// Unix seconds.
  pub checked_at: i64,
}

/// Found lyrics still waiting to be written into their song file.
pub struct UnwrittenLyrics {
  pub track: TrackId,
  pub path: String,
  pub text: String,
}

pub trait LyricsStore: Send + Sync {
  fn lyrics_record(&self, track: TrackId) -> AppResult<Option<LyricsRecord>>;
  fn save_lyrics_record(&self, track: TrackId, record: &LyricsRecord) -> AppResult<()>;
  fn unwritten_lyrics(&self) -> AppResult<Vec<UnwrittenLyrics>>;
  fn mark_lyrics_written(&self, track: TrackId) -> AppResult<()>;
  /// Drops what was found or checked for the song, so its lyrics are looked up afresh.
  fn forget_lyrics(&self, track: TrackId) -> AppResult<()>;
}

/// What a song is looked up by in the online music databases: the title and main artist it is known by now, its file
/// and its length. The album is left out, as a wrong one would hide the song from every search.
pub struct SongLookup<'a> {
  pub title: &'a str,
  pub artist: Option<&'a str>,
  pub path: &'a Path,
  pub duration_ms: u32,
}

impl SongLookup<'_> {
  /// The artist and title as the plain words of a search that takes them in one line.
  pub fn words(&self) -> String {
    self
      .artist
      .map_or_else(|| self.title.to_owned(), |artist| format!("{artist} {}", self.title))
  }
}

/// One online music database that finds the tags of a song.
pub trait TagFinder: Send + Sync {
  fn source(&self) -> TagSource;
  /// The songs the database takes this one for, best first.
  fn find(&self, song: &SongLookup<'_>) -> AppResult<Vec<TagMatch>>;
}

pub trait WebImages: Send + Sync {
  /// The bytes of the image file at `url`.
  fn fetch(&self, url: &str) -> AppResult<Vec<u8>>;
}
