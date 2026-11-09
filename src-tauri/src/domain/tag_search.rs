//! Rules for the tags of a song found in online music databases: which finds can be the same recording, and how the
//! databases' credits and dates turn into the app's tags.

/// How far a found recording's length may lie from the file's for both to be taken as the same song. Releases of one
/// recording differ by a few seconds of silence, while a remix, a live take or another song differs by much more.
pub const CLOSE_LENGTH_MS: u32 = 15_000;

/// Whether a find of `found_ms` can be the recording in a file of `actual_ms`; a find of unknown length can.
pub fn close_length(found_ms: Option<u32>, actual_ms: u32) -> bool {
  found_ms.is_none_or(|found| found.abs_diff(actual_ms) <= CLOSE_LENGTH_MS)
}

/// The letters and digits of a title in lower case, so that spacing, punctuation and case do not tell titles apart.
fn title_letters(title: &str) -> String {
  title
    .chars()
    .filter(|char| char.is_alphanumeric())
    .flat_map(char::to_lowercase)
    .collect()
}

/// Whether a find titled `found` answers a search for `asked`: one title holds the other, as `A Lot` and `A Lot
/// (Remastered)` do, or as a file name `Kino - Blood Type` holds `Blood Type`. Stores that search titles and artists
/// together also return songs that only share a word with the artist, and these do not pass.
pub fn answers_title(found: &str, asked: &str) -> bool {
  let found = title_letters(found);
  let asked = title_letters(asked);
  !found.is_empty() && (found.contains(&asked) || asked.contains(&found))
}

/// Whether `artists` credit the artist `asked`, whatever the spacing, punctuation and case of either spelling.
pub fn credits(artists: &[String], asked: &str) -> bool {
  let asked = title_letters(asked);
  artists.iter().any(|artist| title_letters(artist) == asked)
}

/// What separates the artist from the title in the name of an upload.
const UPLOAD_DASHES: [&str; 3] = [" - ", " – ", " — "];

/// Words of a bracketed note on an upload that tell what kind of upload it is, not which song: such notes go.
const UPLOAD_NOTES: [&str; 11] = [
  "official",
  "audio",
  "video",
  "lyric",
  "lyrics",
  "visualizer",
  "visualiser",
  "hd",
  "hq",
  "4k",
  "mv",
];

/// The title of an upload without its notes on the upload, as `Title (Official Audio) [HD]` gives `Title`; a note
/// that is part of the song's name, such as `(feat. Guest)` or `(Remix)`, stays.
fn without_upload_notes(title: &str) -> String {
  let mut kept = String::new();
  let mut rest = title;
  while let Some(open) = rest.find(['(', '[']) {
    let closer = if rest[open..].starts_with('(') { ')' } else { ']' };
    let Some(length) = rest[open..].find(closer) else {
      break;
    };
    let note = rest[open + 1..open + length].to_lowercase();
    kept.push_str(&rest[..open]);
    let about_upload = note
      .split(|char: char| !char.is_alphanumeric())
      .any(|word| UPLOAD_NOTES.contains(&word));
    if !about_upload {
      kept.push_str(&rest[open..=open + length]);
    }
    rest = &rest[open + length + 1..];
  }
  kept.push_str(rest);
  kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The artist and title of an upload named as songs usually are, `Artist - Title (Official Audio)`.
pub fn upload_credit(name: &str) -> Option<(String, String)> {
  let (artist, title) = UPLOAD_DASHES.iter().find_map(|dash| name.split_once(dash))?;
  let artist = artist.trim();
  let title = without_upload_notes(title);
  (!artist.is_empty() && !title.is_empty()).then(|| (artist.to_owned(), title))
}

/// Words that join a guest to the main artists in a credit written as one line.
const FEATURING: [&str; 4] = [" feat. ", " ft. ", " featuring ", " feat "];

/// The artists of a credit written as one line, such as `A, B & C feat. D`, in order and without repeats.
pub fn split_credit(credit: &str) -> Vec<String> {
  let mut line = credit.to_owned();
  for word in FEATURING {
    line = line.replace(word, ", ");
  }
  let mut names: Vec<String> = Vec::new();
  for name in line.replace(" & ", ", ").split(", ") {
    let name = name.trim();
    if !name.is_empty() && !names.iter().any(|known| known.eq_ignore_ascii_case(name)) {
      names.push(name.to_owned());
    }
  }
  names
}

/// The year a date such as `1988-04-01` or `1988` starts with, when it is a plausible one.
pub fn year_of(date: &str) -> Option<u16> {
  date.get(..4)?.parse().ok().filter(|year| (1000..=9999).contains(year))
}

/// A genre as a tag of a database spells it, such as `post-punk`, with its first letter raised like the genres of
/// song files.
pub fn genre_of(tag: &str) -> Option<String> {
  let tag = tag.trim();
  let mut letters = tag.chars();
  let first = letters.next()?;
  Some(first.to_uppercase().chain(letters).collect())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn an_upload_name_gives_its_artist_and_title() {
    assert_eq!(
      upload_credit("erickD - FALLING (Official Audio) [HD]"),
      Some(("erickD".into(), "FALLING".into()))
    );
    assert_eq!(
      upload_credit("Kino – Blood Type (feat. Guest) (Lyric Video)"),
      Some(("Kino".into(), "Blood Type (feat. Guest)".into()))
    );
    assert_eq!(upload_credit("Fall Effect Festival || ERICKD"), None);
  }

  #[test]
  fn an_artist_is_credited_whatever_the_spelling() {
    let artists = ["Eric Llobet".to_owned(), "erickD".to_owned()];
    assert!(credits(&artists, "ERICKD"));
    assert!(!credits(&artists, "Eric"));
  }

  #[test]
  fn a_credit_splits_into_its_artists() {
    assert_eq!(split_credit("Кино"), ["Кино"]);
    assert_eq!(split_credit("A, B & C feat. D"), ["A", "B", "C", "D"]);
    assert_eq!(
      split_credit("Snakehips ft. Duckwrth & snakehips"),
      ["Snakehips", "Duckwrth"]
    );
  }

  #[test]
  fn a_title_answers_its_versions_but_not_other_songs() {
    assert!(answers_title("a lot", "A Lot"));
    assert!(answers_title("Группа крови (Remastered)", "группа  крови"));
    assert!(answers_title("Группа крови", "Кино - Группа крови"));
    assert!(!answers_title("Viva La Vida", "A Lot"));
    assert!(!answers_title("", "A Lot"));
  }

  #[test]
  fn dates_give_their_year() {
    assert_eq!(year_of("1988-04-01"), Some(1988));
    assert_eq!(year_of("1988"), Some(1988));
    assert_eq!(year_of("88"), None);
    assert_eq!(year_of("0000-01-01"), None);
  }

  #[test]
  fn a_far_length_is_another_recording() {
    assert!(close_length(Some(244_000), 250_000));
    assert!(close_length(None, 250_000));
    assert!(!close_length(Some(400_000), 250_000));
  }

  #[test]
  fn genres_start_with_a_capital() {
    assert_eq!(genre_of("post-punk").as_deref(), Some("Post-punk"));
    assert_eq!(genre_of("  "), None);
  }
}
