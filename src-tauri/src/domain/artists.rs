//! The artists credited on a song: an ordered list with the main artist first, as files and streaming sites name
//! them, and the rule that splits one written-out credit like "A feat. B" into its names.

use super::tags::{TagError, UNKNOWN_ARTIST};

/// Separator of the credit Aio writes into the plain artist tag for players that read only that one.
const CREDIT_SEPARATOR: &str = ", ";

/// Words that bring in a featured artist; they are matched without regard to case and only as whole words.
const FEATURING: &[&str] = &["featuring", "feat.", "feat", "ft.", "ft"];

/// Shortest part a "/" may leave on either side; shorter ones mean a name like AC/DC rather than two artists.
const MIN_SLASH_PART: usize = 3;

/// The artists of one song, never empty, each name trimmed, non-blank and listed once regardless of case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artists(Vec<String>);

impl Artists {
  /// The artists in the given order; blank names are dropped and repeats keep their first place.
  pub fn new<S: AsRef<str>>(names: impl IntoIterator<Item = S>) -> Result<Self, TagError> {
    let mut kept: Vec<String> = Vec::new();
    for name in names {
      let name = name.as_ref().trim();
      if !name.is_empty() && !kept.iter().any(|known| known.to_lowercase() == name.to_lowercase()) {
        kept.push(name.to_owned());
      }
    }
    if kept.is_empty() {
      return Err(TagError::ArtistEmpty);
    }
    Ok(Self(kept))
  }

  /// Splits one written-out credit on the separators that never occur inside a name: "/", ";" and the words that
  /// bring in a featured artist; "&", "," and "x" stay, since names like "Simon & Garfunkel" carry them.
  pub fn parse(credit: &str) -> Result<Self, TagError> {
    let mut names = Vec::new();
    for part in credit.split(';') {
      let (main, featured) = split_featuring(part);
      names.extend(split_slashes(main));
      names.extend(featured.into_iter().flat_map(split_slashes));
    }
    Self::new(names)
  }

  /// Artists for a file that names none.
  pub fn unknown() -> Self {
    Self(vec![UNKNOWN_ARTIST.to_owned()])
  }

  pub fn names(&self) -> &[String] {
    &self.0
  }

  /// The first artist, who owns the song's album when the file names no album artist.
  pub fn main(&self) -> &str {
    &self.0[0]
  }

  /// All names in one line, as written into the plain artist tag and shown where a list does not fit.
  pub fn credit(&self) -> String {
    self.0.join(CREDIT_SEPARATOR)
  }
}

/// The credit before the first featuring word, and the credit after it; brackets around the featured part go.
fn split_featuring(credit: &str) -> (&str, Option<&str>) {
  let whole_word = |at: usize, word: &str| {
    let before = credit[..at].chars().next_back();
    let after = credit[at + word.len()..].chars().next();
    before.is_some_and(|c| c.is_whitespace() || c == '(' || c == '[') && after.is_some_and(char::is_whitespace)
  };
  let found = credit.char_indices().find_map(|(at, _)| {
    FEATURING.iter().find_map(|word| {
      let here = credit[at..].get(..word.len())?;
      (here.eq_ignore_ascii_case(word) && whole_word(at, word)).then_some((at, word.len()))
    })
  });
  let Some((at, len)) = found else {
    return (credit, None);
  };
  let main = credit[..at].trim_end().trim_end_matches(['(', '[']);
  let featured = credit[at + len..].trim().trim_end_matches([')', ']']);
  (main, Some(featured))
}

/// Parts of the credit between slashes, unless a part is too short to be a name of its own.
fn split_slashes(credit: &str) -> Vec<&str> {
  let parts: Vec<&str> = credit.split('/').map(str::trim).collect();
  if parts.len() > 1 && parts.iter().all(|part| part.chars().count() >= MIN_SLASH_PART) {
    parts
  } else {
    vec![credit.trim()]
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn names(credit: &str) -> Vec<String> {
    Artists::parse(credit).unwrap().names().to_vec()
  }

  #[test]
  fn needs_at_least_one_name() {
    assert_eq!(Artists::new(["", "  "]), Err(TagError::ArtistEmpty));
    assert_eq!(Artists::parse(" ; "), Err(TagError::ArtistEmpty));
  }

  #[test]
  fn keeps_order_and_drops_repeats() {
    let artists = Artists::new([" B ", "a", "b", "A"]).unwrap();
    assert_eq!(artists.names(), ["B", "a"]);
    assert_eq!(artists.main(), "B");
    assert_eq!(artists.credit(), "B, a");
  }

  #[test]
  fn splits_on_safe_separators() {
    assert_eq!(
      names("Borneland/Falcon Punch/Line Gøttsche"),
      ["Borneland", "Falcon Punch", "Line Gøttsche"]
    );
    assert_eq!(
      names("Daft Punk; Pharrell Williams"),
      ["Daft Punk", "Pharrell Williams"]
    );
    assert_eq!(names("Calvin Harris feat. Rihanna"), ["Calvin Harris", "Rihanna"]);
    assert_eq!(names("Drake (Ft. Rihanna)"), ["Drake", "Rihanna"]);
    assert_eq!(names("Alpha featuring Bravo/Charlie"), ["Alpha", "Bravo", "Charlie"]);
  }

  #[test]
  fn leaves_names_with_ambiguous_marks_whole() {
    assert_eq!(names("Simon & Garfunkel"), ["Simon & Garfunkel"]);
    assert_eq!(names("Earth, Wind & Fire"), ["Earth, Wind & Fire"]);
    assert_eq!(names("AC/DC"), ["AC/DC"]);
    assert_eq!(names("Lil Nas X"), ["Lil Nas X"]);
    assert_eq!(names("Soft Cell"), ["Soft Cell"]);
    assert_eq!(names("Featurecast"), ["Featurecast"]);
  }
}
