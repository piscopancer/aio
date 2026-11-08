use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::error::{AppError, AppResult};
use super::ports::{AudioFiles, FolderRepository, Notifier, TrackIndex};
use super::views::FolderCheck;
use crate::domain::ids::TrackId;
use crate::domain::library::{FolderPath, LibraryError};

/// Files written per transaction, which also sets how often scan progress is reported.
const BATCH: usize = 50;

/// Library folder management and the scan that keeps the track index in sync with the disk.
pub struct LibraryService {
  folders: Arc<dyn FolderRepository>,
  index: Arc<dyn TrackIndex>,
  files: Arc<dyn AudioFiles>,
  notifier: Arc<dyn Notifier>,
  scanning: AtomicBool,
  rescan: AtomicBool,
}

impl LibraryService {
  pub fn new(
    folders: Arc<dyn FolderRepository>,
    index: Arc<dyn TrackIndex>,
    files: Arc<dyn AudioFiles>,
    notifier: Arc<dyn Notifier>,
  ) -> Self {
    Self {
      folders,
      index,
      files,
      notifier,
      scanning: AtomicBool::new(false),
      rescan: AtomicBool::new(false),
    }
  }

  pub fn add_folder(&self, path: &str) -> AppResult<()> {
    let folder = FolderPath::new(path)?;
    let mut library = self.folders.load_folders()?;
    library.add(folder)?;
    self.folders.save_folders(&library)
  }

  pub fn remove_folder(&self, path: &str) -> AppResult<()> {
    let folder = FolderPath::new(path)?;
    let mut library = self.folders.load_folders()?;
    library.remove(&folder)?;
    self.folders.save_folders(&library)?;
    self.notifier.library_changed();
    Ok(())
  }

  /// Tells whether files saved into `path` would show up in the library, and whether the folder can be added.
  pub fn check_folder(&self, path: &Path) -> AppResult<FolderCheck> {
    let folder = FolderPath::new(path)?;
    let mut library = self.folders.load_folders()?;
    let shown = folder.display();
    if let Some(holder) = library.containing(&folder) {
      return Ok(FolderCheck::InLibrary {
        path: shown,
        library: holder.display(),
      });
    }
    Ok(match library.add(folder) {
      Ok(()) => FolderCheck::WillBeAdded { path: shown },
      Err(LibraryError::Overlaps { existing }) => FolderCheck::Overlaps { path: shown, existing },
      Err(error) => return Err(error.into()),
    })
  }

  /// The library folder that holds `path`, adding `path` as a new library folder (and scanning it) when none does.
  pub fn ensure_folder(&self, path: &Path) -> AppResult<FolderPath> {
    let folder = FolderPath::new(path)?;
    let mut library = self.folders.load_folders()?;
    if let Some(holder) = library.containing(&folder) {
      return Ok(holder.clone());
    }
    library.add(folder.clone())?;
    self.folders.save_folders(&library)?;
    self.scan()?;
    Ok(folder)
  }

  /// Indexes one new file of a library folder at once, without a scan, and returns its track.
  pub fn index_file(&self, folder: &FolderPath, path: &Path) -> AppResult<TrackId> {
    let file = self
      .files
      .file(path)
      .ok_or_else(|| AppError::Fetch(format!("{} was not written", path.display())))?;
    let track = self
      .files
      .read(&file)
      .ok_or_else(|| AppError::Fetch(format!("{} cannot be read as audio", path.display())))?;
    self.index.index_tracks(folder, &[track])?;
    let id = self.index.track_at(&file.path)?;
    self.notifier.library_changed();
    Ok(id)
  }

  /// Indexes new and modified files of every folder and drops vanished ones; a request during a running scan
  /// makes that scan go over the folders once more instead of starting a second one.
  pub fn scan(&self) -> AppResult<()> {
    self.rescan.store(true, Ordering::Release);
    if self.scanning.swap(true, Ordering::AcqRel) {
      return Ok(());
    }
    let mut result = Ok(());
    while self.rescan.swap(false, Ordering::AcqRel) {
      result = self.scan_once();
      // even a failed scan may have committed some batches
      self.notifier.library_changed();
    }
    self.scanning.store(false, Ordering::Release);
    result
  }

  fn scan_once(&self) -> AppResult<()> {
    let library = self.folders.load_folders()?;
    let found: Vec<_> = library
      .folders()
      .iter()
      .map(|folder| (folder, self.files.list(folder)))
      .collect();
    let total = count(found.iter().map(|(_, files)| files.len()).sum());
    let mut done = 0u32;
    self.notifier.scan_progress(done, total);

    for (folder, files) in found {
      let mut indexed: HashMap<String, i64> = self
        .index
        .indexed_files(folder)?
        .into_iter()
        .map(|file| (file.path, file.modified))
        .collect();
      let mut batch = Vec::with_capacity(BATCH);
      for file in files {
        let unchanged = indexed.remove(&file.path) == Some(file.modified);
        if !unchanged && let Some(track) = self.files.read(&file) {
          batch.push(track);
        }
        done += 1;
        if done.is_multiple_of(BATCH as u32) {
          self.index.index_tracks(folder, &batch)?;
          batch.clear();
          self.notifier.scan_progress(done, total);
        }
      }
      self.index.index_tracks(folder, &batch)?;
      let vanished: Vec<String> = indexed.into_keys().collect();
      self.index.drop_files(&vanished)?;
    }
    self.notifier.scan_progress(total, total);
    Ok(())
  }
}

fn count(n: usize) -> u32 {
  u32::try_from(n).unwrap_or(u32::MAX)
}
