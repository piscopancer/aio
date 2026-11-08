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
}

impl LibraryFolders {
  pub fn restore(folders: Vec<FolderPath>) -> Self {
    Self { folders }
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

  pub fn remove(&mut self, folder: &FolderPath) -> Result<(), LibraryError> {
    let index = self
      .folders
      .iter()
      .position(|f| f == folder)
      .ok_or(LibraryError::NotInLibrary)?;
    self.folders.remove(index);
    Ok(())
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
}
