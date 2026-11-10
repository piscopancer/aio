use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use lofty::config::WriteOptions;
use lofty::file::{AudioFile as _, TaggedFile, TaggedFileExt as _};
use lofty::picture::{Picture, PictureType};
use lofty::tag::items::{Timestamp, UNKNOWN_LANGUAGE};
use lofty::tag::{Accessor as _, ItemKey, ItemValue, Tag, TagItem, TagType};
use walkdir::WalkDir;

use super::covers::CoverStore;
use crate::application::error::{AppError, AppResult};
use crate::application::ports::{AlbumShare, AudioFile, AudioFiles, CoverChange, ScannedTrack, SongFiles};
use crate::application::views::TrackTags;
use crate::domain::artists::{Artists, distinct};
use crate::domain::library::FolderPath;
use crate::domain::tags::{ReleaseType, SongTags, is_placeholder};

const AUDIO_EXTENSIONS: &[&str] = &[
  "mp3", "flac", "ogg", "oga", "opus", "m4a", "mp4", "aac", "wav", "aif", "aiff", "wv", "ape",
];

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
    let text = SongText::of(tag, path);
    let album_artists = text.album_artists.or_else(|| {
      text
        .artists
        .as_ref()
        .and_then(|artists| Artists::new([artists.main()]).ok())
    });
    Some(ScannedTrack {
      path: file.path.clone(),
      title: text.title,
      album_artists,
      artists: text.artists,
      album: text.album,
      year: tag.and_then(|t| t.date()).map(|date| u32::from(date.year)),
      release_type: text.release_type,
      track_no: place(tag.and_then(|t| t.track())),
      disc_no: place(tag.and_then(|t| t.disk())),
      genres: text.genres,
      duration_ms: duration_ms(&tagged),
      modified: file.modified,
      cover: tag.and_then(|t| self.save_cover(t)),
    })
  }
}

/// A track or disc number as a file holds it; some taggers write 0 for a song without one, which counts as none.
fn place(number: Option<u32>) -> Option<u32> {
  number.filter(|&number| number > 0)
}

impl SongFiles for LocalAudioFiles {
  fn tags(&self, path: &Path) -> AppResult<TrackTags> {
    let tagged = lofty::read_from_path(path).map_err(|e| unchangeable(path, e))?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let text = SongText::of(tag, path);
    Ok(TrackTags {
      path: path.to_string_lossy().into_owned(),
      title: text.title,
      artists: names(text.artists.as_ref()),
      album_artists: names(text.album_artists.as_ref()),
      album: text.album,
      year: tag.and_then(|t| t.date()).map(|date| date.year),
      release_type: text.release_type,
      track_no: place(tag.and_then(|t| t.track())),
      disc_no: place(tag.and_then(|t| t.disk())),
      genres: text.genres,
      composers: text.composers,
      lyricists: text.lyricists,
      comment: text.comment,
      lyrics: tag.and_then(embedded_lyrics).filter(|text| !text.trim().is_empty()),
      format: path
        .extension()
        .map_or_else(String::new, |ext| ext.to_string_lossy().to_uppercase()),
      bitrate_kbps: tagged.properties().audio_bitrate(),
      duration_ms: duration_ms(&tagged),
    })
  }

  fn write_song(&self, path: &Path, tags: &SongTags, cover: &CoverChange) -> AppResult<()> {
    rewrite(path, |tag| {
      set_song_tags(tag, tags);
      set_cover(tag, cover)
    })
  }

  fn write_album_art(&self, path: &Path, share: &AlbumShare) -> AppResult<()> {
    rewrite(path, |tag| {
      set_year(tag, share.year);
      set_release_type(tag, share.release_type);
      if let Some(owners) = share.owners {
        set_credit(tag, Credit::ALBUM, Artists::new(owners).ok().as_ref());
      }
      set_cover(tag, share.cover)
    })
  }

  fn lyrics(&self, path: &Path) -> AppResult<Option<String>> {
    let tagged = lofty::read_from_path(path).map_err(|e| unchangeable(path, e))?;
    let embedded = tagged
      .primary_tag()
      .or_else(|| tagged.first_tag())
      .and_then(embedded_lyrics);
    // players that keep lyrics out of the tags save them as "<song>.lrc" in the same folder
    let text = embedded.or_else(|| fs::read_to_string(path.with_extension("lrc")).ok());
    Ok(text.filter(|text| !text.trim().is_empty()))
  }

  fn write_lyrics(&self, path: &Path, text: &str) -> AppResult<()> {
    rewrite(path, |tag| {
      // ID3 keeps it in USLT, MP4 in ©lyr and Vorbis comments in LYRICS; synced players read the LRC stamps there
      let key = lyrics_key(tag.tag_type());
      tag.remove_key(key);
      tag.push_unchecked(TagItem::new(key, ItemValue::Text(text.to_owned())));
      Ok(())
    })
  }
}

/// The text tags of a file, trimmed, with blank ones as missing; a file without a title is named after itself.
struct SongText {
  title: String,
  artists: Option<Artists>,
  album_artists: Option<Artists>,
  album: Option<String>,
  release_type: Option<ReleaseType>,
  genres: Vec<String>,
  composers: Vec<String>,
  lyricists: Vec<String>,
  comment: Option<String>,
}

impl SongText {
  fn of(tag: Option<&Tag>, path: &Path) -> Self {
    let text = |read: fn(&Tag) -> Option<String>| {
      tag
        .and_then(read)
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty() && !is_placeholder(s))
    };
    Self {
      title: text(|t| t.title().map(|s| s.into_owned())).unwrap_or_else(|| {
        path
          .file_stem()
          .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
      }),
      artists: tag.and_then(|t| credited(t, Credit::TRACK)),
      album_artists: tag.and_then(|t| credited(t, Credit::ALBUM)),
      album: text(|t| t.album().map(|s| s.into_owned())),
      release_type: tag.and_then(|t| {
        // a tagger puts the primary type first and secondary ones such as `compilation` after it
        listed(t, ItemKey::MusicBrainzReleaseType, &['/', ';']).find_map(ReleaseType::parse)
      }),
      genres: tag.map(|t| names_of(t, ItemKey::Genre)).unwrap_or_default(),
      composers: tag.map(|t| names_of(t, ItemKey::Composer)).unwrap_or_default(),
      lyricists: tag.map(|t| names_of(t, ItemKey::Lyricist)).unwrap_or_default(),
      comment: text(|t| t.comment().map(|s| s.into_owned())),
    }
  }
}

/// The tags one credit of artists is kept in: all names in one line for players that read only that, and the names
/// one by one for those that know the list.
#[derive(Clone, Copy)]
struct Credit {
  line: ItemKey,
  list: ItemKey,
}

impl Credit {
  const TRACK: Self = Self {
    line: ItemKey::TrackArtist,
    list: ItemKey::TrackArtists,
  };
  const ALBUM: Self = Self {
    line: ItemKey::AlbumArtist,
    list: ItemKey::AlbumArtists,
  };
}

/// Every value the tag holds under `key`, split further on `separators`; ID3v2.4 keeps several values of one frame in
/// a single string, split by NUL.
fn listed<'a>(tag: &'a Tag, key: ItemKey, separators: &'a [char]) -> impl Iterator<Item = &'a str> {
  tag
    .get_strings(key)
    .flat_map(|value| value.split('\0'))
    .flat_map(move |value| value.split(separators))
}

/// The distinct values of a tag that lists names or words, such as genres or composers, with placeholders left out;
/// a ";" separates values in files whose format keeps only one.
fn names_of(tag: &Tag, key: ItemKey) -> Vec<String> {
  distinct(listed(tag, key, &[';']).filter(|value| !is_placeholder(value)))
}

/// The names a tagger listed one by one, or else the names in the line credit, each split on the separators that never
/// occur inside a name.
fn credited(tag: &Tag, credit: Credit) -> Option<Artists> {
  let named = listed(tag, credit.list, &[]).filter(|name| !is_placeholder(name));
  Artists::new(named).ok().or_else(|| {
    let names = listed(tag, credit.line, &[])
      .filter_map(|line| Artists::parse(line).ok())
      .flat_map(|artists| artists.names().to_vec())
      .filter(|name| !is_placeholder(name));
    Artists::new(names).ok()
  })
}

/// The names of a credit as the edit dialog lists them; empty for nobody.
fn names(artists: Option<&Artists>) -> Vec<String> {
  artists.map(|artists| artists.names().to_vec()).unwrap_or_default()
}

fn duration_ms(tagged: &TaggedFile) -> u32 {
  u32::try_from(tagged.properties().duration().as_millis()).unwrap_or(u32::MAX)
}

fn unchangeable(path: &Path, error: impl std::fmt::Display) -> AppError {
  AppError::SongFile(format!("{}: {error}", path.display()))
}

/// The file's main tag, added first when the file has none.
pub fn main_tag(file: &mut TaggedFile) -> Option<&mut Tag> {
  if file.primary_tag().is_none() {
    let kind = file.primary_tag_type();
    file.insert_tag(Tag::new(kind));
  }
  file.primary_tag_mut()
}

/// Reads the file, lets `change` edit its main tag and saves it in place.
fn rewrite(path: &Path, change: impl FnOnce(&mut Tag) -> AppResult<()>) -> AppResult<()> {
  let mut file = lofty::read_from_path(path).map_err(|e| unchangeable(path, e))?;
  let tag = main_tag(&mut file).ok_or_else(|| unchangeable(path, "the format cannot hold tags"))?;
  change(tag)?;
  mend_languages(tag);
  file
    .save_to_path(path, WriteOptions::default())
    .map_err(|e| unchangeable(path, explained(&e)))
}

/// The keys lyrics are found under: lofty files ID3's USLT frame under its own key, other formats under the general one.
const LYRICS_KEYS: [ItemKey; 2] = [ItemKey::Lyrics, ItemKey::UnsyncLyrics];

/// The lyrics a tag holds, under whichever key its format keeps them.
fn embedded_lyrics(tag: &Tag) -> Option<String> {
  LYRICS_KEYS
    .into_iter()
    .find_map(|key| tag.get_string(key))
    .map(str::to_owned)
}

/// The key a tag of `kind` keeps lyrics under; ID3 has no frame for the general key and would drop them.
fn lyrics_key(kind: TagType) -> ItemKey {
  match kind {
    TagType::Id3v2 => ItemKey::UnsyncLyrics,
    _ => ItemKey::Lyrics,
  }
}

/// Gives a language to lyrics and comments whose language code is broken, such as the blanks some taggers leave: ID3
/// reads such frames but refuses to write them, which would leave the whole file unchangeable.
fn mend_languages(tag: &mut Tag) {
  let valid = |item: &TagItem| item.lang().iter().all(u8::is_ascii_alphabetic);
  for key in [ItemKey::Lyrics, ItemKey::UnsyncLyrics, ItemKey::Comment] {
    if tag.get_items(key).all(valid) {
      continue;
    }
    let items: Vec<TagItem> = tag.take(key).collect();
    for mut item in items {
      if !valid(&item) {
        item.set_lang(UNKNOWN_LANGUAGE);
      }
      // they came from this tag, and the checked push turns down lyrics, which ID3 keeps in a frame of their own
      tag.push_unchecked(item);
    }
  }
}

/// An error with the causes under it, as lofty names only the outermost step, such as `failed to write Mpeg file`.
fn explained(error: &dyn std::error::Error) -> String {
  let mut text = error.to_string();
  let mut cause = error.source();
  while let Some(inner) = cause {
    text.push_str(": ");
    text.push_str(&inner.to_string());
    cause = inner.source();
  }
  text
}

/// Writes every song tag, removing the ones `tags` leaves empty.
pub fn set_song_tags(tag: &mut Tag, tags: &SongTags) {
  match tags.title() {
    Some(title) => tag.set_title(title.to_owned()),
    None => tag.remove_title(),
  }
  set_credit(tag, Credit::TRACK, tags.artists());
  // the file names whose album it is even when that is the main artist, as other players group albums by it
  set_credit(tag, Credit::ALBUM, Artists::new(tags.album_owners()).ok().as_ref());
  match tags.album() {
    Some(album) => tag.set_album(album.to_owned()),
    None => tag.remove_album(),
  }
  set_year(tag, tags.year());
  set_release_type(tag, tags.release_type());
  match tags.track_no() {
    Some(n) => tag.set_track(n),
    None => tag.remove_track(),
  }
  match tags.disc_no() {
    Some(n) => tag.set_disk(n),
    None => tag.remove_disk(),
  }
  set_values(tag, ItemKey::Genre, tags.genres());
  set_values(tag, ItemKey::Composer, tags.composers());
  set_values(tag, ItemKey::Lyricist, tags.lyricists());
  match tags.comment() {
    Some(comment) => tag.set_comment(comment.to_owned()),
    None => tag.remove_comment(),
  }
  for key in LYRICS_KEYS {
    tag.remove_key(key);
  }
  if let Some(lyrics) = tags.lyrics() {
    // ID3 keeps them in a frame of their own, which the checked push turns down
    tag.push_unchecked(TagItem::new(
      lyrics_key(tag.tag_type()),
      ItemValue::Text(lyrics.to_owned()),
    ));
  }
}

/// Replaces the values under `key` with these, which lofty writes as one field holding several values.
fn set_values<S: AsRef<str>>(tag: &mut Tag, key: ItemKey, values: impl IntoIterator<Item = S>) {
  tag.remove_key(key);
  for value in values {
    tag.push(TagItem::new(key, ItemValue::Text(value.as_ref().to_owned())));
  }
}

/// Writes `artists` as one line, and name by name as well when there are several; nobody removes both.
fn set_credit(tag: &mut Tag, credit: Credit, artists: Option<&Artists>) {
  tag.remove_key(credit.list);
  match artists {
    Some(artists) => {
      tag.insert_text(credit.line, artists.credit());
      if artists.names().len() > 1 {
        set_values(tag, credit.list, artists.names());
      }
    }
    None => tag.remove_key(credit.line),
  }
}

/// Puts the primary release type first and keeps the secondary ones a tagger wrote after it, such as `compilation`.
fn set_release_type(tag: &mut Tag, release_type: Option<ReleaseType>) {
  let key = ItemKey::MusicBrainzReleaseType;
  let secondary: Vec<String> = listed(tag, key, &['/', ';'])
    .map(str::trim)
    .filter(|value| !value.is_empty() && ReleaseType::parse(value).is_none())
    .map(str::to_owned)
    .collect();
  let primary = release_type.map(|kind| kind.tag().to_owned());
  set_values(tag, key, primary.into_iter().chain(secondary));
}

fn set_year(tag: &mut Tag, year: Option<u16>) {
  match year {
    Some(year) => tag.set_date(Timestamp {
      year,
      ..Timestamp::default()
    }),
    None => tag.remove_date(),
  }
}

/// Replaces the front cover with the image file `cover` names, or drops every picture so no other one stands in.
fn set_cover(tag: &mut Tag, cover: &CoverChange) -> AppResult<()> {
  match cover {
    CoverChange::Keep => {}
    CoverChange::Remove => {
      while !tag.pictures().is_empty() {
        tag.remove_picture(0);
      }
    }
    CoverChange::Set { path } => {
      let image = Path::new(path);
      let mut reader = fs::File::open(image).map_err(|e| AppError::Image(format!("{}: {e}", image.display())))?;
      let mut picture = Picture::from_reader(&mut reader).map_err(|e| AppError::Image(e.to_string()))?;
      picture.set_pic_type(PictureType::CoverFront);
      tag.remove_picture_type(PictureType::CoverFront);
      tag.push_picture(picture);
    }
  }
  Ok(())
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn blank_lyrics_languages_are_mended() {
    let mut tag = Tag::new(TagType::Id3v2);
    let mut lyrics = TagItem::new(ItemKey::UnsyncLyrics, ItemValue::Text("la la".to_owned()));
    lyrics.set_lang(*b"   ");
    tag.push_unchecked(lyrics);
    tag.push(TagItem::new(ItemKey::Comment, ItemValue::Text("fine".to_owned())));
    mend_languages(&mut tag);
    assert!(
      tag.items().all(|item| item.lang().iter().all(u8::is_ascii_alphabetic)),
      "{:?}",
      tag.items().map(TagItem::lang).collect::<Vec<_>>()
    );
    assert_eq!(tag.get_string(ItemKey::UnsyncLyrics), Some("la la"));
  }

  /// A short silent MP3 without tags: MPEG-1 layer III frames at 128 kbps and 44.1 kHz, 417 bytes each.
  fn silent_mp3(path: &Path) {
    let mut frame = vec![0u8; 417];
    frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x64]);
    fs::write(path, frame.repeat(20)).unwrap();
  }

  #[test]
  fn lists_and_album_parts_survive_a_round_trip() {
    let dir = std::env::temp_dir().join(format!("aio-tags-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("song.mp3");
    silent_mp3(&path);
    let files = LocalAudioFiles::new(dir.join("covers"));
    rewrite(&path, |tag| {
      // a secondary type a tagger wrote stays behind the primary one
      tag.push(TagItem::new(
        ItemKey::MusicBrainzReleaseType,
        ItemValue::Text("compilation".to_owned()),
      ));
      Ok(())
    })
    .unwrap();
    let tags = SongTags::new("Song", Artists::new(["Jay"]).unwrap(), Some("Throne"), Some(2011))
      .unwrap()
      .with_album_artists(Some(Artists::new(["Jay", "Ye"]).unwrap()))
      .with_release_type(Some(ReleaseType::Ep))
      .with_genres(["Rap", "Soul"])
      .with_writers(["Jay", "Ye"], ["Frank"])
      .with_comment(Some("first take"))
      .with_lyrics(Some("[00:01.00]la la"));
    files.write_song(&path, &tags, &CoverChange::Keep).unwrap();

    let read = files.tags(&path).unwrap();
    assert_eq!(read.album_artists, ["Jay", "Ye"]);
    assert_eq!(read.release_type, Some(ReleaseType::Ep));
    assert_eq!(read.genres, ["Rap", "Soul"]);
    assert_eq!(
      (read.composers, read.lyricists),
      (vec!["Jay".to_owned(), "Ye".to_owned()], vec!["Frank".to_owned()])
    );
    assert_eq!(read.comment.as_deref(), Some("first take"));
    assert_eq!(read.lyrics.as_deref(), Some("[00:01.00]la la"));
    files
      .write_song(&path, &tags.clone().with_lyrics(Some("  ")), &CoverChange::Keep)
      .unwrap();
    assert_eq!(files.tags(&path).unwrap().lyrics, None);
    let tagged = lofty::read_from_path(&path).unwrap();
    let kinds: Vec<&str> = listed(tagged.primary_tag().unwrap(), ItemKey::MusicBrainzReleaseType, &['/']).collect();
    assert_eq!(kinds, ["ep", "compilation"]);

    let owners = ["Ye".to_owned()];
    let share = AlbumShare {
      year: Some(2011),
      release_type: Some(ReleaseType::Single),
      owners: Some(&owners),
      cover: &CoverChange::Keep,
    };
    files.write_album_art(&path, &share).unwrap();
    let read = files.tags(&path).unwrap();
    assert_eq!(
      (read.album_artists, read.release_type),
      (vec!["Ye".to_owned()], Some(ReleaseType::Single))
    );
    assert_eq!(read.genres, ["Rap", "Soul"]);
    fs::remove_dir_all(&dir).unwrap();
  }
}
