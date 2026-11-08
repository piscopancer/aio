//! Song lyrics as LRC text holds them: lines stamped with the moment they are sung, sometimes with a stamp per word,
//! or plain text without any timing.

/// How far two lengths of a song may differ and still be the same recording; a remaster with a longer intro is not.
pub const SAME_RECORDING_MS: u32 = 3000;

/// Whether lyrics timed for a song of `matched_ms` fit a file of `actual_ms`.
pub fn same_recording(matched_ms: u32, actual_ms: u32) -> bool {
  matched_ms.abs_diff(actual_ms) <= SAME_RECORDING_MS
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
  pub start_ms: u32,
  pub text: String,
}

/// One sung line; `words` is empty unless the text times every word, and `text` is empty for an instrumental gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
  pub start_ms: u32,
  pub text: String,
  pub words: Vec<Word>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lyrics {
  /// Lines in the order they are sung.
  Synced(Vec<Line>),
  Plain(String),
}

impl Lyrics {
  /// Reads LRC text, or plain text when no line carries a time stamp; blank text holds no lyrics.
  pub fn parse(text: &str) -> Option<Self> {
    let offset = text.lines().find_map(offset_of).unwrap_or(0);
    let mut lines: Vec<Line> = Vec::new();
    for raw in text.lines() {
      let (stamps, rest) = leading_stamps(raw.trim());
      if stamps.is_empty() {
        continue;
      }
      let (text, words) = words_of(rest, offset);
      for start in stamps {
        lines.push(Line {
          start_ms: shift(start, offset),
          text: text.clone(),
          words: words.clone(),
        });
      }
    }
    if lines.iter().any(|line| !line.text.is_empty()) {
      lines.sort_by_key(|line| line.start_ms);
      return Some(Self::Synced(lines));
    }
    let plain = text
      .lines()
      .filter(|line| !is_tag_line(line.trim()) && leading_stamps(line.trim()).0.is_empty())
      .collect::<Vec<_>>()
      .join("\n");
    let plain = plain.trim();
    (!plain.is_empty()).then(|| Self::Plain(plain.to_owned()))
  }
}

/// The time stamps a line opens with, such as `[01:02.34][01:40.00]`, and the text after them.
fn leading_stamps(line: &str) -> (Vec<u32>, &str) {
  let mut stamps = Vec::new();
  let mut rest = line;
  while let Some(inner) = rest.strip_prefix('[') {
    let Some((stamp, after)) = inner.split_once(']') else {
      break;
    };
    let Some(ms) = stamp_ms(stamp) else {
      break;
    };
    stamps.push(ms);
    rest = after;
  }
  (stamps, rest.trim())
}

/// `mm:ss`, `mm:ss.x`, `mm:ss.xx` or `mm:ss.xxx`, also with a colon before the fraction.
fn stamp_ms(stamp: &str) -> Option<u32> {
  let (minutes, rest) = stamp.split_once(':')?;
  let (seconds, fraction) = match rest.split_once(['.', ':']) {
    Some((seconds, fraction)) => (seconds, fraction),
    None => (rest, ""),
  };
  let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
  if !digits(minutes) || !digits(seconds) || !(fraction.is_empty() || digits(fraction)) || fraction.len() > 3 {
    return None;
  }
  let fraction_ms = match fraction.len() {
    0 => 0,
    1 => fraction.parse::<u32>().ok()? * 100,
    2 => fraction.parse::<u32>().ok()? * 10,
    _ => fraction.parse::<u32>().ok()?,
  };
  Some(minutes.parse::<u32>().ok()? * 60_000 + seconds.parse::<u32>().ok()? * 1000 + fraction_ms)
}

/// The words of enhanced LRC, `<00:01.00>Hello <00:01.50>world`, and the line's text without the stamps.
fn words_of(text: &str, offset: i64) -> (String, Vec<Word>) {
  if !text.contains('<') {
    return (text.to_owned(), Vec::new());
  }
  let mut words = Vec::new();
  for piece in text.split('<').filter(|piece| !piece.is_empty()) {
    if let Some((stamp, word)) = piece.split_once('>')
      && let Some(ms) = stamp_ms(stamp)
    {
      if !word.trim().is_empty() {
        words.push(Word {
          start_ms: shift(ms, offset),
          text: word.to_owned(),
        });
      }
    } else {
      // text before the first stamp, or a stray angle bracket, is not timed
      return (text.to_owned(), Vec::new());
    }
  }
  let line = words
    .iter()
    .map(|w| w.text.as_str())
    .collect::<String>()
    .trim()
    .to_owned();
  (line, words)
}

/// The `[offset:+250]` tag: milliseconds every stamp is moved earlier by.
fn offset_of(line: &str) -> Option<i64> {
  let value = line.trim().strip_prefix("[offset:")?.strip_suffix(']')?;
  value.trim().trim_start_matches('+').parse().ok()
}

fn shift(ms: u32, offset: i64) -> u32 {
  u32::try_from((i64::from(ms) - offset).max(0)).unwrap_or(u32::MAX)
}

/// An LRC header such as `[ar:Artist]`, which is no part of plain lyrics.
fn is_tag_line(line: &str) -> bool {
  line.starts_with('[')
    && line.ends_with(']')
    && line[1..]
      .split_once(':')
      .is_some_and(|(key, _)| key.chars().all(char::is_alphabetic))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn synced(text: &str) -> Vec<Line> {
    match Lyrics::parse(text) {
      Some(Lyrics::Synced(lines)) => lines,
      other => panic!("expected synced lyrics, got {other:?}"),
    }
  }

  #[test]
  fn reads_timed_lines_in_sung_order() {
    let lines = synced("[ar:Someone]\n[00:12.50]Second\n[00:01.00]First\n[01:02.345][01:10]Twice\n");
    let times: Vec<u32> = lines.iter().map(|l| l.start_ms).collect();
    assert_eq!(times, [1000, 12_500, 62_345, 70_000]);
    assert_eq!(lines[0].text, "First");
    assert_eq!(lines[3].text, "Twice");
  }

  #[test]
  fn keeps_empty_lines_as_gaps_and_applies_the_offset() {
    let lines = synced("[offset:+500]\n[00:01.00]Hi\n[00:05.00]\n");
    assert_eq!(lines[0].start_ms, 500);
    assert_eq!(lines[1].text, "");
  }

  #[test]
  fn reads_word_stamps() {
    let lines = synced("[00:01.00]<00:01.00>Hello <00:01.50>world");
    assert_eq!(lines[0].text, "Hello world");
    assert_eq!(lines[0].words.len(), 2);
    assert_eq!(lines[0].words[1].start_ms, 1500);
  }

  #[test]
  fn text_without_stamps_is_plain_and_blank_text_is_nothing() {
    assert_eq!(
      Lyrics::parse("[ti:Song]\nLa la\nla"),
      Some(Lyrics::Plain("La la\nla".into()))
    );
    assert_eq!(Lyrics::parse("  \n"), None);
    assert_eq!(Lyrics::parse("[00:01.00]\n[00:02.00]"), None);
  }

  #[test]
  fn tells_recordings_apart_by_length() {
    assert!(same_recording(180_000, 182_500));
    assert!(!same_recording(180_000, 190_000));
  }
}
