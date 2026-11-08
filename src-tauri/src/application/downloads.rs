use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use super::error::{AppError, AppResult};
use super::library::LibraryService;
use super::playlists::PlaylistService;
use super::ports::{DownloadSettingsStore, DownloadTools, FetchJob, FetchProgress, MediaSource, Notifier};
use super::views::*;
use crate::domain::download::{
  AudioQuality, Candidate, SongLink, SongTags, closest_match, guess_tags, unique_file_name,
};
use crate::domain::ids::{DownloadId, PlaylistId};

/// How long a check for newer download tools counts as fresh.
const UPDATE_EVERY_SECS: i64 = 24 * 60 * 60;
/// Bitrate assumed for the original stream when the site does not report one; YouTube's AAC stream is about this.
const FALLBACK_KBPS: u32 = 128;

/// What the user confirmed in the download dialog.
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRequest {
  pub video_url: String,
  pub cover: Option<CoverSource>,
  pub title: String,
  pub artist: String,
  pub album: Option<String>,
  pub year: Option<u16>,
  pub quality: AudioQuality,
  /// A folder for this download only; null uses the folder from the settings.
  pub folder: Option<String>,
  pub playlist_ids: Vec<PlaylistId>,
}

/// Fetching songs from streaming sites into the library: the tools, previews and the running downloads.
pub struct DownloadService {
  media: Arc<dyn MediaSource>,
  tools: Arc<dyn DownloadTools>,
  settings: Arc<dyn DownloadSettingsStore>,
  library: Arc<LibraryService>,
  playlists: Arc<PlaylistService>,
  notifier: Arc<dyn Notifier>,
  /// The system Music folder, the default target.
  music_dir: PathBuf,
  downloads: Mutex<Vec<DownloadView>>,
  tools_view: Mutex<ToolsView>,
  /// Held while tools are installed or updated, so two dialogs never fetch the same tool at once.
  tools_busy: Mutex<()>,
  next_id: AtomicU32,
}

impl DownloadService {
  pub fn new(
    media: Arc<dyn MediaSource>,
    tools: Arc<dyn DownloadTools>,
    settings: Arc<dyn DownloadSettingsStore>,
    library: Arc<LibraryService>,
    playlists: Arc<PlaylistService>,
    notifier: Arc<dyn Notifier>,
    music_dir: PathBuf,
  ) -> Self {
    Self {
      media,
      tools,
      settings,
      library,
      playlists,
      notifier,
      music_dir,
      downloads: Mutex::new(Vec::new()),
      tools_view: Mutex::new(ToolsView::default()),
      tools_busy: Mutex::new(()),
      next_id: AtomicU32::new(1),
    }
  }

  pub fn settings(&self) -> DownloadSettingsView {
    DownloadSettingsView {
      folder: self.default_folder().to_string_lossy().into_owned(),
    }
  }

  /// Saves the folder downloads go to; null goes back to the system Music folder.
  pub fn set_folder(&self, folder: Option<String>) -> AppResult<DownloadSettingsView> {
    let mut settings = self.settings.download_settings();
    settings.folder = folder.filter(|f| !f.trim().is_empty());
    self.settings.save_download_settings(&settings)?;
    Ok(self.settings())
  }

  fn default_folder(&self) -> PathBuf {
    self
      .settings
      .download_settings()
      .folder
      .map_or_else(|| self.music_dir.clone(), PathBuf::from)
  }

  fn target_folder(&self, folder: Option<&str>) -> PathBuf {
    folder.map_or_else(|| self.default_folder(), PathBuf::from)
  }

  pub fn check_folder(&self, folder: Option<String>) -> AppResult<FolderCheck> {
    self.library.check_folder(&self.target_folder(folder.as_deref()))
  }

  pub fn tools(&self) -> ToolsView {
    let mut view = lock(&self.tools_view).clone();
    view.versions = self.tools.versions();
    view.checked_at = self
      .settings
      .download_settings()
      .tools_checked_at
      .and_then(|t| u32::try_from(t).ok());
    view
  }

  fn set_tools(&self, change: impl FnOnce(&mut ToolsView)) {
    change(&mut lock(&self.tools_view));
    self.notifier.tools_changed(&self.tools());
  }

  /// Installs missing tools, then checks for a newer extractor in the background once a day.
  pub fn prepare(self: &Arc<Self>) -> AppResult<ToolsView> {
    self.install_missing()?;
    let checked = self.settings.download_settings().tools_checked_at.unwrap_or(0);
    if now() - checked >= UPDATE_EVERY_SECS {
      let service = self.clone();
      thread::spawn(move || {
        if let Err(error) = service.update_tools() {
          eprintln!("updating download tools failed: {error}");
        }
      });
    }
    Ok(self.tools())
  }

  fn install_missing(&self) -> AppResult<()> {
    if self.tools.versions().complete() {
      return Ok(());
    }
    let _busy = lock(&self.tools_busy);
    let result = self.tools.install_missing(&mut |install| {
      self.set_tools(|view| view.installing = Some(install));
    });
    self.set_tools(|view| view.installing = None);
    result
  }

  pub fn update_tools(&self) -> AppResult<ToolsView> {
    self.install_missing()?;
    let _busy = lock(&self.tools_busy);
    self.set_tools(|view| view.updating = true);
    let result = self.tools.update();
    if result.is_ok() {
      let mut settings = self.settings.download_settings();
      settings.tools_checked_at = Some(now());
      self.settings.save_download_settings(&settings)?;
    }
    self.set_tools(|view| view.updating = false);
    result.map(|()| self.tools())
  }

  /// Resolves a pasted link to the video the audio would come from, with guessed tags and the size per quality.
  pub fn preview(&self, link: &str) -> AppResult<DownloadPreview> {
    let song = SongLink::parse(link)?;
    self.install_missing()?;
    let preview = if song.has_audio() {
      let video = self.media.video(&song.url())?;
      let (title, artist, album) = guess_tags(&video.facts);
      DownloadPreview {
        service: song.service(),
        video_url: video.url,
        cover_url: video.thumbnail,
        duration_ms: video.duration_ms,
        title,
        artist,
        album,
        year: video.year,
        qualities: qualities(video.original_kbps, video.duration_ms),
      }
    } else {
      let track = self.media.spotify_track(song.id())?;
      let query = format!("{} {}", track.artists.join(" "), track.title);
      let candidates: Vec<Candidate> = self.media.search_songs(&query)?;
      let best = closest_match(candidates, track.duration_ms)?;
      let video = self.media.video(&best.url)?;
      DownloadPreview {
        service: song.service(),
        video_url: video.url,
        cover_url: track.cover.or(video.thumbnail),
        duration_ms: track.duration_ms,
        title: track.title,
        artist: track.artists.join(", "),
        album: track.album.or(video.facts.album),
        year: track.year.or(video.year),
        qualities: qualities(video.original_kbps, video.duration_ms),
      }
    };
    Ok(preview)
  }

  pub fn downloads(&self) -> Vec<DownloadView> {
    lock(&self.downloads).clone()
  }

  /// Validates the request and runs the download on its own thread; progress arrives as events.
  pub fn start(self: &Arc<Self>, request: DownloadRequest) -> AppResult<DownloadId> {
    let song = SongLink::parse(&request.video_url)?;
    if !song.has_audio() {
      return Err(AppError::Fetch(
        "the song must be matched to a YouTube video first".into(),
      ));
    }
    let tags = SongTags::new(&request.title, &request.artist, request.album.as_deref(), request.year)?;
    let folder = self.target_folder(request.folder.as_deref());
    // fails early when the folder can never join the library
    if let FolderCheck::Overlaps { existing, .. } = self.library.check_folder(&folder)? {
      return Err(crate::domain::library::LibraryError::Overlaps { existing }.into());
    }
    let id = DownloadId(self.next_id.fetch_add(1, Ordering::Relaxed));
    self.update(|downloads| {
      downloads.push(DownloadView {
        id,
        title: tags.title().to_owned(),
        artist: tags.artist().to_owned(),
        cover: request.cover.clone(),
        folder: folder.to_string_lossy().into_owned(),
        stage: DownloadStage::Queued,
      });
    });
    let service = self.clone();
    let url = song.url();
    thread::spawn(move || {
      let stage = match service.run(id, &url, &tags, &request, &folder) {
        Ok(track_id) => DownloadStage::Done { track_id },
        Err(error) => DownloadStage::Failed { error },
      };
      service.set_stage(id, stage);
    });
    Ok(id)
  }

  /// Removes a finished or failed download from the list.
  pub fn dismiss(&self, id: DownloadId) {
    self.update(|downloads| {
      downloads.retain(|d| d.id != id || matches!(d.stage, DownloadStage::Queued | DownloadStage::Downloading { .. }));
    });
  }

  fn run(
    &self,
    id: DownloadId,
    url: &str,
    tags: &SongTags,
    request: &DownloadRequest,
    folder: &Path,
  ) -> AppResult<crate::domain::ids::TrackId> {
    self.install_missing()?;
    std::fs::create_dir_all(folder).map_err(|e| AppError::Fetch(format!("cannot create {}: {e}", folder.display())))?;
    let name = unique_file_name(&tags.file_stem(), request.quality.extension(), |name| {
      folder.join(name).exists()
    });
    let target = folder.join(name);
    let job = FetchJob {
      video_url: url,
      quality: request.quality,
      target: &target,
    };
    if let Err(first) = self.fetch(id, &job) {
      // an outdated extractor is the usual reason; update it and try once more
      eprintln!("download failed, updating yt-dlp and retrying: {first}");
      if self.update_tools().is_err() {
        return Err(first);
      }
      self.fetch(id, &job)?;
    }
    self.set_stage(id, DownloadStage::Tagging);
    self.media.write_tags(&target, tags, request.cover.as_ref())?;
    let library_folder = self.library.ensure_folder(folder)?;
    let track = self.library.index_file(&library_folder, &target)?;
    for playlist in &request.playlist_ids {
      self.playlists.add_tracks(*playlist, &[track])?;
    }
    self.notifier.library_changed();
    Ok(track)
  }

  fn fetch(&self, id: DownloadId, job: &FetchJob<'_>) -> AppResult<()> {
    // whole percents, so a download sends about a hundred updates at most
    let mut reported = 0u8;
    self.set_stage(id, DownloadStage::Downloading { percent: 0 });
    self.media.fetch(job, &mut |progress| match progress {
      FetchProgress::Downloading(fraction) => {
        let percent = (fraction * 100.0).round().clamp(0.0, 100.0) as u8;
        if percent != reported {
          reported = percent;
          self.set_stage(id, DownloadStage::Downloading { percent });
        }
      }
      FetchProgress::Converting => self.set_stage(id, DownloadStage::Converting),
    })
  }

  fn set_stage(&self, id: DownloadId, stage: DownloadStage) {
    self.update(|downloads| {
      if let Some(download) = downloads.iter_mut().find(|d| d.id == id) {
        download.stage = stage;
      }
    });
  }

  fn update(&self, change: impl FnOnce(&mut Vec<DownloadView>)) {
    let snapshot = {
      let mut downloads = lock(&self.downloads);
      change(&mut downloads);
      downloads.clone()
    };
    self.notifier.downloads_changed(&snapshot);
  }
}

fn qualities(original_kbps: Option<u32>, duration_ms: u32) -> Vec<QualityOption> {
  let original = original_kbps.unwrap_or(FALLBACK_KBPS);
  AudioQuality::ALL
    .into_iter()
    .map(|quality| QualityOption {
      quality,
      kbps: quality.kbps(original),
      estimated_bytes: u32::try_from(quality.estimated_bytes(original, duration_ms)).unwrap_or(u32::MAX),
    })
    .collect()
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
  // a panicked download thread leaves plain data behind, which is still usable
  mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn now() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}
