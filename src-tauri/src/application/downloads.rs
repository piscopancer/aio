use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use super::error::{AppError, AppResult};
use super::library::LibraryService;
use super::playlists::PlaylistService;
use super::ports::{
  DownloadOutcome, DownloadStore, DownloadTools, FetchJob, FetchProgress, MediaSource, Notifier, SavedDownload,
  SongFinder,
};
use super::views::*;
use crate::domain::artists::Artists;
use crate::domain::download::{AudioQuality, Candidate, SongLink, closest_match, guess_tags, unique_file_name};
use crate::domain::ids::{DownloadId, PlaylistId, TrackId};
use crate::domain::tags::SongTags;

/// How long a check for newer download tools counts as fresh.
const UPDATE_EVERY_SECS: i64 = 24 * 60 * 60;
/// Bitrate assumed for the original stream when the site does not report one; YouTube's AAC stream is about this.
const FALLBACK_KBPS: u32 = 128;

/// What the user confirmed in the download dialog.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRequest {
  pub video_url: String,
  pub cover: Option<CoverSource>,
  pub title: String,
  pub artists: Vec<String>,
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
  finder: Arc<dyn SongFinder>,
  tools: Arc<dyn DownloadTools>,
  /// The download settings and the kept list.
  store: Arc<dyn DownloadStore>,
  library: Arc<LibraryService>,
  playlists: Arc<PlaylistService>,
  notifier: Arc<dyn Notifier>,
  /// The system Music folder, the default target.
  music_dir: PathBuf,
  downloads: Mutex<Vec<DownloadView>>,
  /// The cancel flag of each download still running.
  cancels: Mutex<HashMap<DownloadId, Arc<AtomicBool>>>,
  tools_view: Mutex<ToolsView>,
  /// Held while tools are installed or updated, so two dialogs never fetch the same tool at once.
  tools_busy: Mutex<()>,
}

impl DownloadService {
  pub fn new(
    media: Arc<dyn MediaSource>,
    finder: Arc<dyn SongFinder>,
    tools: Arc<dyn DownloadTools>,
    store: Arc<dyn DownloadStore>,
    library: Arc<LibraryService>,
    playlists: Arc<PlaylistService>,
    notifier: Arc<dyn Notifier>,
    music_dir: PathBuf,
  ) -> Self {
    Self {
      media,
      finder,
      tools,
      store,
      library,
      playlists,
      notifier,
      music_dir,
      downloads: Mutex::new(Vec::new()),
      cancels: Mutex::new(HashMap::new()),
      tools_view: Mutex::new(ToolsView::default()),
      tools_busy: Mutex::new(()),
    }
  }

  pub fn settings(&self) -> DownloadSettingsView {
    DownloadSettingsView {
      folder: self.default_folder().to_string_lossy().into_owned(),
    }
  }

  /// Saves the folder downloads go to; null goes back to the system Music folder.
  pub fn set_folder(&self, folder: Option<String>) -> AppResult<DownloadSettingsView> {
    let mut settings = self.store.download_settings();
    settings.folder = folder.filter(|f| !f.trim().is_empty());
    self.store.save_download_settings(&settings)?;
    Ok(self.settings())
  }

  fn default_folder(&self) -> PathBuf {
    self
      .store
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
      .store
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
    let checked = self.store.download_settings().tools_checked_at.unwrap_or(0);
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
      let mut settings = self.store.download_settings();
      settings.tools_checked_at = Some(now());
      self.store.save_download_settings(&settings)?;
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
        // a video names its artists in one line, which splits like a file's artist tag
        artists: Artists::parse(&artist)
          .map(|artists| artists.names().to_vec())
          .unwrap_or_default(),
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
        artists: track.artists,
        album: track.album.or(video.facts.album),
        year: track.year.or(video.year),
        qualities: qualities(video.original_kbps, video.duration_ms),
      }
    };
    Ok(preview)
  }

  /// Songs and videos found by a typed name, for the user to pick the one to download.
  pub fn search(&self, query: &str) -> AppResult<SongFinds> {
    self.finder.find_songs(query.trim())
  }

  /// The address the dialog streams a found song from, so it can be heard before it is downloaded.
  pub fn stream(&self, link: &str) -> AppResult<String> {
    let song = SongLink::parse(link)?;
    self.install_missing()?;
    self.media.audio_stream(&song.url())
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
    let tags = tags_of(&request)?;
    let folder = self.target_folder(request.folder.as_deref());
    // fails early when the folder can never join the library
    if let FolderCheck::Overlaps { existing, .. } = self.library.check_folder(&folder)? {
      return Err(crate::domain::library::LibraryError::Overlaps { existing }.into());
    }
    let id = self.store.add_download(&request, &folder)?;
    self.update(|downloads| downloads.push(view(id, &tags, &request, &folder, DownloadStage::Queued)));
    self.launch(Job {
      id,
      url: song.url(),
      tags,
      request,
      folder,
      file_name: None,
    });
    Ok(id)
  }

  /// Brings back the list of the last session and resumes the downloads quitting cut short; yt-dlp carries on from the partial file each of them left.
  pub fn resume(self: &Arc<Self>) {
    let saved = self.store.saved_downloads().unwrap_or_else(|error| {
      eprintln!("cannot read the saved downloads: {error}");
      Vec::new()
    });
    for SavedDownload {
      id,
      request,
      folder,
      file_name,
      outcome,
    } in saved
    {
      let (Ok(tags), Ok(song)) = (tags_of(&request), SongLink::parse(&request.video_url)) else {
        continue;
      };
      let stage = match outcome {
        Some(DownloadOutcome::Done(track_id)) => DownloadStage::Done { track_id },
        Some(DownloadOutcome::Failed(error)) => DownloadStage::Failed {
          error: AppError::Fetch(error),
        },
        None => DownloadStage::Queued,
      };
      let running = matches!(stage, DownloadStage::Queued);
      self.update(|downloads| downloads.push(view(id, &tags, &request, &folder, stage)));
      if running {
        self.launch(Job {
          id,
          url: song.url(),
          tags,
          request,
          folder,
          file_name,
        });
      }
    }
  }

  /// Runs a kept download on its own thread.
  fn launch(self: &Arc<Self>, job: Job) {
    let id = job.id;
    let cancelled = Arc::new(AtomicBool::new(false));
    lock(&self.cancels).insert(id, cancelled.clone());
    let service = self.clone();
    thread::spawn(move || {
      let result = service.run(job, &cancelled);
      lock(&service.cancels).remove(&id);
      // a cancelled download has already left the list
      if !cancelled.load(Ordering::Relaxed) {
        let outcome = match &result {
          Ok(track_id) => DownloadOutcome::Done(*track_id),
          Err(error) => DownloadOutcome::Failed(error.to_string()),
        };
        if let Err(error) = service.store.end_download(id, &outcome) {
          eprintln!("cannot save how download {} ended: {error}", id.0);
        }
        service.set_stage(
          id,
          match result {
            Ok(track_id) => DownloadStage::Done { track_id },
            Err(error) => DownloadStage::Failed { error },
          },
        );
      }
    });
  }

  /// Stops a running download, removes what it wrote so far and takes it off the list.
  pub fn cancel(&self, id: DownloadId) {
    if let Some(cancelled) = lock(&self.cancels).get(&id) {
      cancelled.store(true, Ordering::Relaxed);
    }
    self.update(|downloads| downloads.retain(|d| d.id != id));
    self.forget(id);
  }

  /// Removes a finished or failed download from the list.
  pub fn dismiss(&self, id: DownloadId) {
    let mut removed = false;
    self.update(|downloads| {
      let before = downloads.len();
      downloads.retain(|d| d.id != id || matches!(d.stage, DownloadStage::Queued | DownloadStage::Downloading { .. }));
      removed = downloads.len() < before;
    });
    if removed {
      self.forget(id);
    }
  }

  fn forget(&self, id: DownloadId) {
    if let Err(error) = self.store.remove_download(id) {
      eprintln!("cannot remove download {} from the saved list: {error}", id.0);
    }
  }

  fn run(&self, job: Job, cancelled: &AtomicBool) -> AppResult<TrackId> {
    let Job {
      id,
      url,
      tags,
      request,
      folder,
      file_name,
    } = job;
    self.install_missing()?;
    std::fs::create_dir_all(&folder)
      .map_err(|e| AppError::Fetch(format!("cannot create {}: {e}", folder.display())))?;
    let name = file_name.unwrap_or_else(|| {
      let name = unique_file_name(&tags.file_stem(), request.quality.extension(), |name| {
        folder.join(name).exists()
      });
      // kept, so that a run cut short by quitting resumes into the same partial file rather than starting a new one
      if let Err(error) = self.store.set_download_file(id, &name) {
        eprintln!("cannot save the file name of download {}: {error}", id.0);
      }
      name
    });
    let target = folder.join(name);
    let job = FetchJob {
      video_url: &url,
      quality: request.quality,
      target: &target,
      cancelled,
    };
    if let Err(first) = self.fetch(id, &job) {
      if cancelled.load(Ordering::Relaxed) {
        return Err(first);
      }
      // an outdated extractor is the usual reason; update it and try once more
      eprintln!("download failed, updating yt-dlp and retrying: {first}");
      if self.update_tools().is_err() {
        return Err(first);
      }
      self.fetch(id, &job)?;
    }
    if cancelled.load(Ordering::Relaxed) {
      let _ = std::fs::remove_file(&target);
      return Err(AppError::Fetch("the download was cancelled".into()));
    }
    self.set_stage(id, DownloadStage::Tagging);
    self.media.write_tags(&target, &tags, request.cover.as_ref())?;
    let library_folder = self.library.ensure_folder(&folder)?;
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
    self.set_stage(
      id,
      DownloadStage::Downloading {
        percent: 0,
        total_bytes: None,
      },
    );
    self.media.fetch(job, &mut |progress| match progress {
      FetchProgress::Downloading { fraction, total_bytes } => {
        let percent = (fraction * 100.0).round().clamp(0.0, 100.0) as u8;
        if percent != reported {
          reported = percent;
          self.set_stage(
            id,
            DownloadStage::Downloading {
              percent,
              total_bytes: Some(total_bytes),
            },
          );
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

/// One run of a kept download.
struct Job {
  id: DownloadId,
  url: String,
  tags: SongTags,
  request: DownloadRequest,
  folder: PathBuf,
  /// The file an earlier run picked, so this one resumes into it.
  file_name: Option<String>,
}

/// The tags a new song gets from the request.
fn tags_of(request: &DownloadRequest) -> AppResult<SongTags> {
  Ok(SongTags::new(
    &request.title,
    Artists::new(&request.artists)?,
    request.album.as_deref(),
    request.year,
  )?)
}

/// The row of the list a download shows.
fn view(
  id: DownloadId,
  tags: &SongTags,
  request: &DownloadRequest,
  folder: &Path,
  stage: DownloadStage,
) -> DownloadView {
  DownloadView {
    id,
    video_url: request.video_url.clone(),
    // a new song always has both, `SongTags::new` sees to it
    title: tags.title().unwrap_or_default().to_owned(),
    artist: tags.artists().map(Artists::credit).unwrap_or_default(),
    cover: request.cover.clone(),
    folder: folder.to_string_lossy().into_owned(),
    stage,
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

/// Unix seconds now.
pub fn now() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}
