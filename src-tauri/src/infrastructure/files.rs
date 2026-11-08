use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use lofty::file::{AudioFile as _, TaggedFileExt as _};
use lofty::picture::PictureType;
use lofty::tag::Accessor as _;
use lofty::tag::{ItemKey, Tag};
use walkdir::WalkDir;

use super::covers::CoverStore;
use crate::application::ports::{AudioFile, AudioFiles, ScannedTrack};
use crate::domain::library::FolderPath;

const AUDIO_EXTENSIONS: &[&str] = &[
  "mp3", "flac", "ogg", "oga", "opus", "m4a", "mp4", "aac", "wav", "aif", "aiff", "wv", "ape",
];

const UNKNOWN_ARTIST: &str = "Unknown Artist";
const UNKNOWN_ALBUM: &str = "Unknown Album";

/// Audio files on the local disk, read with lofty; embedded covers are extracted into the cover store.
pub struct LocalAudioFiles {
  covers: CoverStore,
}

impl LocalAudioFiles {
  pub fn new(covers_dir: PathBuf) -> Self {
    Self {
      covers: CoverStore::new(covers_dir),
    }
  }

  /// Writes the front cover (or else any picture) under its content hash, so albums sharing art share the file.
  fn save_cover(&self, tag: &Tag) -> Option<String> {
    let pictures = tag.pictures();
    let picture = pictures
      .iter()
      .find(|p| p.pic_type() == PictureType::CoverFront)
      .or_else(|| pictures.first())?;
    let ext = picture.mime_type().and_then(|mime| mime.ext()).unwrap_or("jpg");
    let path = self.covers.store(picture.data(), ext).ok()?;
    Some(path.to_string_lossy().into_owned())
  }
}

impl AudioFiles for LocalAudioFiles {
  fn list(&self, folder: &FolderPath) -> Vec<AudioFile> {
    WalkDir::new(folder.as_path())
      .follow_links(true)
      .into_iter()
      .filter_map(Result::ok)
      .filter(|entry| entry.file_type().is_file() && is_audio(entry.path()))
      .map(|entry| AudioFile {
        path: entry.path().to_string_lossy().into_owned(),
        modified: modified(entry.metadata().ok()),
      })
      .collect()
  }

  fn file(&self, path: &Path) -> Option<AudioFile> {
    let meta = fs::metadata(path).ok().filter(|meta| meta.is_file())?;
    Some(AudioFile {
      path: path.to_string_lossy().into_owned(),
      modified: modified(Some(meta)),
    })
  }

  fn read(&self, file: &AudioFile) -> Option<ScannedTrack> {
    let path = Path::new(&file.path);
    let tagged = lofty::read_from_path(path)
      .inspect_err(|error| eprintln!("skipping {}: {error}", file.path))
      .ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let text = |read: fn(&Tag) -> Option<String>| {
      tag
        .and_then(read)
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
    };

    let title = text(|t| t.title().map(|s| s.into_owned())).unwrap_or_else(|| {
      path
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
    });
    let artist = text(|t| t.artist().map(|s| s.into_owned())).unwrap_or_else(|| UNKNOWN_ARTIST.to_owned());
    let album_artist =
      text(|t| t.get_string(ItemKey::AlbumArtist).map(str::to_owned)).unwrap_or_else(|| artist.clone());
    let album = text(|t| t.album().map(|s| s.into_owned())).unwrap_or_else(|| UNKNOWN_ALBUM.to_owned());

    Some(ScannedTrack {
      path: file.path.clone(),
      title,
      artist,
      album,
      album_artist,
      year: tag.and_then(|t| t.date()).map(|date| u32::from(date.year)),
      track_no: tag.and_then(|t| t.track()),
      disc_no: tag.and_then(|t| t.disk()),
      duration_ms: u32::try_from(tagged.properties().duration().as_millis()).unwrap_or(u32::MAX),
      modified: file.modified,
      cover: tag.and_then(|t| self.save_cover(t)),
    })
  }
}

/// Modification time in Unix seconds, the stamp that tells a scan whether a file changed.
fn modified(meta: Option<fs::Metadata>) -> i64 {
  meta
    .and_then(|meta| meta.modified().ok())
    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
    .map_or(0, |duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
}

fn is_audio(path: &Path) -> bool {
  path
    .extension()
    .and_then(|ext| ext.to_str())
    .is_some_and(|ext| AUDIO_EXTENSIONS.iter().any(|known| known.eq_ignore_ascii_case(ext)))
}
