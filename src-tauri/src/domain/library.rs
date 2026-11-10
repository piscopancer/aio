use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LibraryError {
  #[error("the folder path must be absolute")]
  NotAbsolute,
  #[error("this folder is already in the library")]
  AlreadyAdded,
  #[error("this folder overlaps {existing}, which is already in the library")]
  Overlaps { existing: String },
  #[error("this folder is not in the library")]
  NotInLibrary,
  #[error("the system music folder stays in the library; it can be hidden instead")]
  SystemFolder,
}

/// Absolute, normalized path of a library folder; it is the folder's identity.
#[derive(Debug, Clone)]
pub struct FolderPath(PathBuf);

impl FolderPath {
  pub fn new(path: impl AsRef<Path>) -> Result<Self, LibraryError> {
    let path = path.as_ref();
    if !path.is_absolute() {
      return Err(LibraryError::NotAbsolute);
    }
    let normalized = path
      .components()
      .filter(|c| !matches!(c, Component::CurDir))
      .collect::<PathBuf>();
    Ok(Self(normalized))
  }

  pub fn as_path(&self) -> &Path {
    &self.0
  }

  pub fn display(&self) -> String {
    self.0.to_string_lossy().into_owned()
  }

  /// Path components compared the way the platform's file system compares them.
  fn key(&self) -> Vec<String> {
    self
      .0
      .components()
      .map(|c| {
        let part = c.as_os_str().to_string_lossy();
        if cfg!(windows) {
          part.to_lowercase()
        } else {
          part.into_owned()
        }
      })
      .collect()
  }

  /// True when one folder contains the other or both are the same folder.
  fn overlaps(&self, other: &Self) -> bool {
    let (a, b) = (self.key(), other.key());
    a.starts_with(&b) || b.starts_with(&a)
  }

  /// True when `other` is this folder or sits somewhere inside it.
  fn holds(&self, other: &Self) -> bool {
    other.key().starts_with(&self.key())
  }
}

impl PartialEq for FolderPath {
  fn eq(&self, other: &Self) -> bool {
    self.key() == other.key()
  }
}

impl Eq for FolderPath {}

/// Aggregate of the folders the library is built from; it guarantees no file is reachable through two folders.
#[derive(Debug, Clone, Default)]
pub struct LibraryFolders {
  folders: Vec<FolderPath>,
  /// The system music folder, which the library always reads, either as a folder of its own or through one holding it.
  system: Option<FolderPath>,
}

impl LibraryFolders {
  pub fn restore(folders: Vec<FolderPath>) -> Self {
    Self { folders, system: None }
  }

  /// Makes the library always read `system`, the system music folder: unless a folder of the library already holds it, it is added and the folders inside it merge into it; true when it was added.
  pub fn keep_system(&mut self, system: FolderPath) -> bool {
    let added = self.containing(&system).is_none();
    if added {
      self.folders.retain(|folder| !system.holds(folder));
      self.folders.push(system.clone());
    }
    self.system = Some(system);
    added
  }

  /// True when `folder` is the system music folder, which can be hidden but not removed.
  pub fn is_system(&self, folder: &FolderPath) -> bool {
    self.system.as_ref() == Some(folder)
  }

  pub fn folders(&self) -> &[FolderPath] {
    &self.folders
  }

  pub fn add(&mut self, folder: FolderPath) -> Result<(), LibraryError> {
    if self.folders.contains(&folder) {
      return Err(LibraryError::AlreadyAdded);
    }
    if let Some(existing) = self.folders.iter().find(|f| f.overlaps(&folder)) {
      return Err(LibraryError::Overlaps {
        existing: existing.display(),
      });
    }
    self.folders.push(folder);
    Ok(())
  }

  /// The library folder that holds `folder`, which may be that folder itself.
  pub fn containing(&self, folder: &FolderPath) -> Option<&FolderPath> {
    let key = folder.key();
    self.folders.iter().find(|f| key.starts_with(&f.key()))
  }

  /// Removes a folder other than the system one; true when the system folder, which the removed folder held, was added back in its place.
  pub fn remove(&mut self, folder: &FolderPath) -> Result<bool, LibraryError> {
    if self.is_system(folder) {
      return Err(LibraryError::SystemFolder);
    }
    let index = self
      .folders
      .iter()
      .position(|f| f == folder)
      .ok_or(LibraryError::NotInLibrary)?;
    self.folders.remove(index);
    Ok(self.system.clone().is_some_and(|system| self.keep_system(system)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn path(p: &str) -> FolderPath {
    let root = if cfg!(windows) { "C:\\" } else { "/" };
    FolderPath::new(format!("{root}{p}")).unwrap()
  }

  #[test]
  fn rejects_relative_paths() {
    assert_eq!(FolderPath::new("music").unwrap_err(), LibraryError::NotAbsolute);
  }

  #[test]
  fn rejects_the_same_folder_twice() {
    let mut library = LibraryFolders::default();
    library.add(path("music")).unwrap();
    assert_eq!(library.add(path("music")).unwrap_err(), LibraryError::AlreadyAdded);
  }

  #[test]
  fn rejects_parent_and_child_folders() {
    let mut library = LibraryFolders::default();
    library.add(path("music/rock")).unwrap();
    assert!(matches!(library.add(path("music")), Err(LibraryError::Overlaps { .. })));
    assert!(matches!(
      library.add(path("music/rock/live")),
      Err(LibraryError::Overlaps { .. })
    ));
  }

  #[test]
  fn allows_siblings_with_a_common_prefix() {
    let mut library = LibraryFolders::default();
    library.add(path("music/rock")).unwrap();
    library.add(path("music/rockabilly")).unwrap();
    assert_eq!(library.folders().len(), 2);
  }

  #[test]
  fn finds_the_folder_holding_a_subfolder() {
    let mut library = LibraryFolders::default();
    library.add(path("music")).unwrap();
    assert_eq!(library.containing(&path("music/new")), Some(&path("music")));
    assert_eq!(library.containing(&path("music")), Some(&path("music")));
    assert_eq!(library.containing(&path("musicals")), None);
  }

  #[test]
  fn removing_an_unknown_folder_fails() {
    let mut library = LibraryFolders::default();
    assert_eq!(library.remove(&path("music")).unwrap_err(), LibraryError::NotInLibrary);
  }

  #[test]
  fn the_system_folder_takes_in_the_folders_inside_it() {
    let mut library = LibraryFolders::restore(vec![path("music/rock"), path("films")]);
    assert!(library.keep_system(path("music")));
    assert_eq!(library.folders(), [path("films"), path("music")]);
  }

  #[test]
  fn the_system_folder_is_not_added_under_a_folder_holding_it() {
    let mut library = LibraryFolders::restore(vec![path("home")]);
    assert!(!library.keep_system(path("home/music")));
    assert_eq!(library.folders(), [path("home")]);
  }

  #[test]
  fn the_system_folder_cannot_be_removed() {
    let mut library = LibraryFolders::default();
    library.keep_system(path("music"));
    assert_eq!(library.remove(&path("music")).unwrap_err(), LibraryError::SystemFolder);
  }

  #[test]
  fn the_system_folder_comes_back_when_the_folder_holding_it_leaves() {
    let mut library = LibraryFolders::restore(vec![path("home")]);
    library.keep_system(path("home/music"));
    assert_eq!(library.remove(&path("home")), Ok(true));
    assert_eq!(library.folders(), [path("home/music")]);
  }
}
