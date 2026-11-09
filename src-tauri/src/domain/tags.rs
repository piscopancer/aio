//! Tags of a song file and who owns them: a song owns its title, artists, numbers and genre, while the cover and the
//! year belong to its album, so every song of one album carries the same ones.

use serde::Serialize;
use specta::Type;

use super::artists::Artists;

/// Words that rippers and players write into a tag they know nothing about, such as `Неизв. исполнитель` in the
/// album artist of a ripped CD, compared in lower case.
const PLACEHOLDERS: [&str; 10] = [
  "unknown",
  "unknown artist",
  "unknown album",
  "unknown genre",
  "неизвестно",
  "неизв. исполнитель",
  "неизвестный исполнитель",
  "неизв. альбом",
  "неизвестный альбом",
  "неизвестный жанр",
];

/// Whether a tag holds a placeholder instead of a value; such a tag counts as empty, so that a song is not filed
/// under an artist or album named `Unknown`.
pub fn is_placeholder(value: &str) -> bool {
  PLACEHOLDERS.contains(&value.trim().to_lowercase().as_str())
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TagError {
  #[error("the song needs a title")]
  TitleEmpty,
  #[error("the song needs an artist")]
  ArtistEmpty,
  #[error("a year has four digits")]
  YearInvalid,
  #[error("track and disc numbers go from 1 to {max}")]
  NumberInvalid { max: u32 },
}

/// Tags written into a song file; blank text means no tag, and a song without a title or artist is listed under its
/// file name and credited to nobody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongTags {
  title: Option<String>,
  artists: Option<Artists>,
  album_artist: Option<String>,
  album: Option<String>,
  year: Option<u16>,
  track_no: Option<u32>,
  disc_no: Option<u32>,
  genre: Option<String>,
}

/// Trimmed text, or nothing when it is blank.
fn text(value: Option<&str>) -> Option<String> {
  value.map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned)
}

impl SongTags {
  /// Highest track or disc number a song can carry.
  pub const MAX_NUMBER: u32 = 999;

  /// Tags of a new song, which needs a title and artists to be named by.
  pub fn new(title: &str, artists: Artists, album: Option<&str>, year: Option<u16>) -> Result<Self, TagError> {
    let tags = Self::edited(title, Some(artists), album, year)?;
    if tags.title.is_none() {
      return Err(TagError::TitleEmpty);
    }
    Ok(tags)
  }

  /// Tags as the user edits them, where the title and the artists may be left blank to clear them.
  pub fn edited(
    title: &str,
    artists: Option<Artists>,
    album: Option<&str>,
    year: Option<u16>,
  ) -> Result<Self, TagError> {
    if year.is_some_and(|year| !(1000..=9999).contains(&year)) {
      return Err(TagError::YearInvalid);
    }
    Ok(Self {
      title: text(Some(title)),
      artists,
      album_artist: None,
      album: text(album),
      year,
      track_no: None,
      disc_no: None,
      genre: None,
    })
  }

  /// The same tags with the song's place on its album.
  pub fn numbered(self, track_no: Option<u32>, disc_no: Option<u32>) -> Result<Self, TagError> {
    let valid = |n: Option<u32>| n.is_none_or(|n| (1..=Self::MAX_NUMBER).contains(&n));
    if !valid(track_no) || !valid(disc_no) {
      return Err(TagError::NumberInvalid { max: Self::MAX_NUMBER });
    }
    Ok(Self {
      track_no,
      disc_no,
      ..self
    })
  }

  pub fn with_genre(self, genre: Option<&str>) -> Self {
    Self {
      genre: text(genre),
      ..self
    }
  }

  /// The same tags with an album artist other than the song's main artist, as compilations and features have.
  pub fn with_album_artist(self, album_artist: Option<&str>) -> Self {
    let album_artist = text(album_artist).filter(|name| Some(name.as_str()) != self.main_artist());
    Self { album_artist, ..self }
  }

  /// The same tags with the album's year, which wins over the song's own.
  pub fn with_year(self, year: Option<u16>) -> Self {
    Self { year, ..self }
  }

  pub fn title(&self) -> Option<&str> {
    self.title.as_deref()
  }

  pub fn artists(&self) -> Option<&Artists> {
    self.artists.as_ref()
  }

  /// The artist the library lists the song under: the first credited one, if any.
  pub fn main_artist(&self) -> Option<&str> {
    self.artists.as_ref().map(Artists::main)
  }

  /// The album artist written only when it differs from the main artist.
  pub fn album_artist(&self) -> Option<&str> {
    self.album_artist.as_deref()
  }

  pub fn album(&self) -> Option<&str> {
    self.album.as_deref()
  }

  pub fn year(&self) -> Option<u16> {
    self.year
  }

  pub fn track_no(&self) -> Option<u32> {
    self.track_no
  }

  pub fn disc_no(&self) -> Option<u32> {
    self.disc_no
  }

  pub fn genre(&self) -> Option<&str> {
    self.genre.as_deref()
  }

  /// Title and artist of the album the library files the song under, the pair that identifies an album; a song that
  /// names no album is on none, and an album whose song names no artist belongs to nobody.
  pub fn album_key(&self) -> Option<(&str, Option<&str>)> {
    Some((self.album()?, self.album_artist().or(self.main_artist())))
  }

  /// "Artists - Title", or the title alone when no artist is credited, with the characters Windows forbids in file
  /// names replaced.
  pub fn file_stem(&self) -> String {
    const MAX_CHARS: usize = 150;
    let title = self.title().unwrap_or_default();
    let raw = match &self.artists {
      Some(artists) => format!("{} - {title}", artists.credit()),
      None => title.to_owned(),
    };
    let cleaned: String = raw
      .chars()
      .map(|c| {
        if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
          '_'
        } else {
          c
        }
      })
      .take(MAX_CHARS)
      .collect();
    cleaned.trim_end_matches(['.', ' ']).to_owned()
  }
}

/// Where an edited song ends up, which decides whose cover and year it carries afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumMove<A> {
  /// It stays on its album, so a new cover or year applies to every song of that album.
  Stays,
  /// It moves onto another album that exists, and takes that album's cover and year.
  Joins(A),
  /// It starts an album of its own, keeping the cover and year it was given.
  Starts,
}

/// The move of a song from album `current` to the album its new tags name, `existing` when the library has one.
pub fn album_move<A: PartialEq>(current: A, existing: Option<A>) -> AlbumMove<A> {
  match existing {
    Some(album) if album == current => AlbumMove::Stays,
    Some(album) => AlbumMove::Joins(album),
    None => AlbumMove::Starts,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn placeholders_count_as_empty() {
    assert!(is_placeholder("Неизв. исполнитель"));
    assert!(is_placeholder(" unknown album "));
    assert!(!is_placeholder("Кино"));
  }

  fn by(names: &[&str]) -> Artists {
    Artists::new(names).unwrap()
  }

  #[test]
  fn tags_need_a_title() {
    assert_eq!(SongTags::new(" ", by(&["x"]), None, None), Err(TagError::TitleEmpty));
    assert_eq!(SongTags::new("x", by(&["y"]), Some("  "), None).unwrap().album(), None);
    assert_eq!(
      SongTags::new("x", by(&["y"]), None, Some(99)),
      Err(TagError::YearInvalid)
    );
    assert_eq!(
      SongTags::new("x", by(&["y"]), None, Some(1993)).unwrap().year(),
      Some(1993)
    );
  }

  #[test]
  fn numbers_stay_in_range() {
    let tags = || SongTags::new("x", by(&["y"]), None, None).unwrap();
    assert!(tags().numbered(Some(1), Some(999)).is_ok());
    assert_eq!(
      tags().numbered(Some(0), None),
      Err(TagError::NumberInvalid { max: 999 })
    );
    assert!(tags().numbered(None, Some(1000)).is_err());
  }

  #[test]
  fn album_key_falls_back_like_the_library() {
    let tags = SongTags::new("x", by(&["Artist", "Guest"]), None, None).unwrap();
    assert_eq!(tags.album_key(), None);
    let tags = SongTags::new("x", by(&["Artist", "Guest"]), Some("Hits"), None).unwrap();
    assert_eq!(tags.album_key(), Some(("Hits", Some("Artist"))));
    let nobody = SongTags::edited("x", None, Some("Tape"), None).unwrap();
    assert_eq!(nobody.album_key(), Some(("Tape", None)));
    let tags = SongTags::new("x", by(&["Artist"]), Some("Hits"), None)
      .unwrap()
      .with_album_artist(Some("Various"));
    assert_eq!(tags.album_key(), Some(("Hits", Some("Various"))));
    let same = SongTags::new("x", by(&["Artist", "Guest"]), None, None)
      .unwrap()
      .with_album_artist(Some("Artist"));
    assert_eq!(same.album_artist(), None);
  }

  #[test]
  fn file_names_are_safe() {
    let tags = SongTags::new(" Thunder? ", by(&["AC/DC", "Guest"]), None, None).unwrap();
    assert_eq!(tags.file_stem(), "AC_DC, Guest - Thunder_");
    let nobody = SongTags::edited("Intro", None, None, None).unwrap();
    assert_eq!(nobody.file_stem(), "Intro");
  }

  #[test]
  fn songs_take_the_art_of_the_album_they_join() {
    assert_eq!(album_move(1, Some(1)), AlbumMove::Stays);
    assert_eq!(album_move(1, Some(2)), AlbumMove::Joins(2));
    assert_eq!(album_move(1, None), AlbumMove::Starts);
  }
}
