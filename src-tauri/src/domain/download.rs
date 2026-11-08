//! Rules of fetching a song from a streaming service into the library: which links are accepted, which quality
//! makes which file, how tags are guessed from a video and how the file is named.

use serde::{Deserialize, Serialize};
use specta::Type;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DownloadError {
  #[error("this is not a YouTube, YouTube Music or Spotify song link")]
  UnsupportedLink,
  #[error("playlist, album and artist links are not supported yet, paste a link to one song")]
  CollectionLink,
  #[error("the song needs a title")]
  TitleEmpty,
  #[error("the song needs an artist")]
  ArtistEmpty,
  #[error("a year has four digits")]
  YearInvalid,
  #[error("no matching song was found on YouTube Music")]
  NoMatch,
  #[error("downloading is not available on this operating system")]
  UnsupportedPlatform,
}

/// Where a song link points to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Service {
  #[serde(rename = "youtube")]
  YouTube,
  #[serde(rename = "youtubeMusic")]
  YouTubeMusic,
  Spotify,
}

/// A validated link to exactly one song; the id is the service's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongLink {
  service: Service,
  id: String,
}

impl SongLink {
  pub fn parse(text: &str) -> Result<Self, DownloadError> {
    let text = text.trim();
    if let Some(id) = text.strip_prefix("spotify:track:") {
      return Self::spotify(id);
    }
    if text.starts_with("spotify:") {
      return Err(DownloadError::CollectionLink);
    }
    let url = Url::parse(text)
      .or_else(|_| Url::parse(&format!("https://{text}")))
      .map_err(|_| DownloadError::UnsupportedLink)?;
    let host = url.host_str().ok_or(DownloadError::UnsupportedLink)?;
    let host = host.strip_prefix("www.").unwrap_or(host);
    let mut segments = url.path_segments().into_iter().flatten().filter(|s| !s.is_empty());
    match host {
      "youtu.be" => Self::youtube(Service::YouTube, segments.next()),
      "youtube.com" | "m.youtube.com" | "music.youtube.com" => {
        let service = if host == "music.youtube.com" {
          Service::YouTubeMusic
        } else {
          Service::YouTube
        };
        match segments.next() {
          Some("watch") => Self::youtube(service, query(&url, "v").as_deref()),
          Some("shorts" | "embed" | "live") => Self::youtube(service, segments.next()),
          Some("playlist" | "channel" | "browse") => Err(DownloadError::CollectionLink),
          Some(s) if s.starts_with('@') => Err(DownloadError::CollectionLink),
          _ => Err(DownloadError::UnsupportedLink),
        }
      }
      "open.spotify.com" | "play.spotify.com" => {
        // localized links put a language segment such as intl-de first
        let kind = segments.find(|s| !s.starts_with("intl-"));
        match kind {
          Some("track") => Self::spotify(segments.next().unwrap_or_default()),
          Some("album" | "playlist" | "artist" | "show" | "episode") => Err(DownloadError::CollectionLink),
          _ => Err(DownloadError::UnsupportedLink),
        }
      }
      _ => Err(DownloadError::UnsupportedLink),
    }
  }

  fn youtube(service: Service, id: Option<&str>) -> Result<Self, DownloadError> {
    let id = id.ok_or(DownloadError::UnsupportedLink)?;
    let valid = id.len() == 11 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !valid {
      return Err(DownloadError::UnsupportedLink);
    }
    Ok(Self {
      service,
      id: id.to_owned(),
    })
  }

  fn spotify(id: &str) -> Result<Self, DownloadError> {
    if id.len() != 22 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
      return Err(DownloadError::UnsupportedLink);
    }
    Ok(Self {
      service: Service::Spotify,
      id: id.to_owned(),
    })
  }

  pub fn service(&self) -> Service {
    self.service
  }

  pub fn id(&self) -> &str {
    &self.id
  }

  /// Canonical address of the song, without tracking or playlist parameters.
  pub fn url(&self) -> String {
    match self.service {
      Service::YouTube => format!("https://www.youtube.com/watch?v={}", self.id),
      Service::YouTubeMusic => format!("https://music.youtube.com/watch?v={}", self.id),
      Service::Spotify => format!("https://open.spotify.com/track/{}", self.id),
    }
  }

  /// True when the audio itself can be fetched from this link; Spotify links have to be matched on YouTube first.
  pub fn has_audio(&self) -> bool {
    self.service != Service::Spotify
  }
}

fn query(url: &Url, key: &str) -> Option<String> {
  url
    .query_pairs()
    .find(|(k, _)| k == key)
    .map(|(_, value)| value.into_owned())
}

/// What file a download produces: the original AAC stream as it is, or MP3 re-encoded at a fixed bitrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum AudioQuality {
  #[serde(rename = "original")]
  Original,
  #[serde(rename = "mp3-320")]
  Mp3High,
  #[serde(rename = "mp3-192")]
  Mp3Medium,
  #[serde(rename = "mp3-128")]
  Mp3Low,
}

impl AudioQuality {
  pub const ALL: [Self; 4] = [Self::Original, Self::Mp3High, Self::Mp3Medium, Self::Mp3Low];

  pub fn extension(self) -> &'static str {
    match self {
      Self::Original => "m4a",
      _ => "mp3",
    }
  }

  /// Bitrate of the resulting file, given the bitrate of the original stream.
  pub fn kbps(self, original_kbps: u32) -> u32 {
    match self {
      Self::Original => original_kbps,
      Self::Mp3High => 320,
      Self::Mp3Medium => 192,
      Self::Mp3Low => 128,
    }
  }

  /// Approximate file size: bitrate times duration, ignoring the container and the cover.
  pub fn estimated_bytes(self, original_kbps: u32, duration_ms: u32) -> u64 {
    u64::from(self.kbps(original_kbps)) * u64::from(duration_ms) / 8
  }
}

/// Title, artist and album written into a downloaded file; title and artist are never blank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongTags {
  title: String,
  artist: String,
  album: Option<String>,
  year: Option<u16>,
}

impl SongTags {
  pub fn new(title: &str, artist: &str, album: Option<&str>, year: Option<u16>) -> Result<Self, DownloadError> {
    let title = title.trim();
    let artist = artist.trim();
    if title.is_empty() {
      return Err(DownloadError::TitleEmpty);
    }
    if artist.is_empty() {
      return Err(DownloadError::ArtistEmpty);
    }
    if year.is_some_and(|year| !(1000..=9999).contains(&year)) {
      return Err(DownloadError::YearInvalid);
    }
    Ok(Self {
      title: title.to_owned(),
      artist: artist.to_owned(),
      album: album.map(str::trim).filter(|a| !a.is_empty()).map(str::to_owned),
      year,
    })
  }

  pub fn title(&self) -> &str {
    &self.title
  }

  pub fn artist(&self) -> &str {
    &self.artist
  }

  pub fn album(&self) -> Option<&str> {
    self.album.as_deref()
  }

  pub fn year(&self) -> Option<u16> {
    self.year
  }

  /// "Artist - Title" with the characters Windows forbids in file names replaced.
  pub fn file_stem(&self) -> String {
    const MAX_CHARS: usize = 150;
    let raw = format!("{} - {}", self.artist, self.title);
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

/// File name in a folder that does not clash: "stem.ext", else "stem (2).ext", "stem (3).ext" and so on.
pub fn unique_file_name(stem: &str, extension: &str, taken: impl Fn(&str) -> bool) -> String {
  let first = format!("{stem}.{extension}");
  if !taken(&first) {
    return first;
  }
  (2u32..)
    .map(|n| format!("{stem} ({n}).{extension}"))
    .find(|name| !taken(name))
    .unwrap_or(first)
}

/// The year at the start of a date such as `1993-05-14` or `19930514`.
pub fn year_of(date: &str) -> Option<u16> {
  date.get(..4)?.parse().ok().filter(|year| (1000..=9999).contains(year))
}

/// What a video page tells about the song in it; YouTube Music fills artist, track and album, plain videos only
/// the title and channel.
#[derive(Debug, Clone, Default)]
pub struct VideoFacts {
  pub title: String,
  pub channel: String,
  pub artist: Option<String>,
  pub track: Option<String>,
  pub album: Option<String>,
}

/// Words that mark a bracketed part of a video title as noise rather than part of the song name.
const NOISE: &[&str] = &[
  "official",
  "video",
  "audio",
  "lyric",
  "lyrics",
  "visualizer",
  "visualiser",
  "music video",
  "mv",
  "hd",
  "hq",
  "4k",
  "explicit",
  "clean",
];

/// Best guess of title, artist and album: real music tags first, then "Artist - Title" in the video title with
/// noise like "(Official Video)" removed, then the channel name as the artist.
pub fn guess_tags(facts: &VideoFacts) -> (String, String, Option<String>) {
  let album = facts.album.clone().filter(|a| !a.trim().is_empty());
  if let (Some(artist), Some(track)) = (&facts.artist, &facts.track)
    && !artist.trim().is_empty()
    && !track.trim().is_empty()
  {
    return (track.trim().to_owned(), artist.trim().to_owned(), album);
  }
  let title = strip_noise(&facts.title);
  let title = title.split(" | ").next().unwrap_or(&title).trim().to_owned();
  for dash in [" - ", " – ", " — "] {
    if let Some((artist, song)) = title.split_once(dash)
      && !artist.trim().is_empty()
      && !song.trim().is_empty()
    {
      return (song.trim().to_owned(), artist.trim().to_owned(), album);
    }
  }
  (title, channel_artist(&facts.channel), album)
}

/// Removes bracketed parts made only of noise words, keeping ones like "(feat. X)" or "(Remix)".
fn strip_noise(title: &str) -> String {
  let mut out = String::with_capacity(title.len());
  let mut rest = title;
  while let Some(start) = rest.find(['(', '[']) {
    let close = if rest[start..].starts_with('(') { ')' } else { ']' };
    let Some(len) = rest[start..].find(close) else {
      break;
    };
    let inner = rest[start + 1..start + len].to_lowercase();
    let noisy = inner
      .split(|c: char| !c.is_alphanumeric())
      .filter(|w| !w.is_empty())
      .all(|w| NOISE.contains(&w) || w == "music" || w == "official");
    out.push_str(&rest[..start]);
    if !noisy {
      out.push_str(&rest[start..=start + len]);
    }
    rest = &rest[start + len + 1..];
  }
  out.push_str(rest);
  out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn channel_artist(channel: &str) -> String {
  let channel = channel.trim();
  let channel = channel.strip_suffix(" - Topic").unwrap_or(channel);
  let channel = channel.strip_suffix("VEVO").unwrap_or(channel);
  channel.trim().to_owned()
}

/// A search result that might be the song looked for.
#[derive(Debug, Clone)]
pub struct Candidate {
  pub url: String,
  pub duration_ms: Option<u32>,
}

/// Picks the search result whose length is closest to the song's; results without a length come last.
pub fn closest_match(candidates: Vec<Candidate>, duration_ms: u32) -> Result<Candidate, DownloadError> {
  candidates
    .into_iter()
    .enumerate()
    .min_by_key(|(rank, candidate)| {
      let distance = candidate.duration_ms.map_or(u32::MAX, |d| d.abs_diff(duration_ms));
      (distance, *rank)
    })
    .map(|(_, candidate)| candidate)
    .ok_or(DownloadError::NoMatch)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn accepts_every_youtube_song_link() {
    for link in [
      "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PL123&t=4",
      "youtube.com/watch?v=dQw4w9WgXcQ",
      "https://youtu.be/dQw4w9WgXcQ?si=abc",
      "https://m.youtube.com/shorts/dQw4w9WgXcQ",
    ] {
      let song = SongLink::parse(link).unwrap();
      assert_eq!(song.service(), Service::YouTube);
      assert_eq!(song.id(), "dQw4w9WgXcQ");
    }
    let music = SongLink::parse("https://music.youtube.com/watch?v=dQw4w9WgXcQ&feature=share").unwrap();
    assert_eq!(music.service(), Service::YouTubeMusic);
  }

  #[test]
  fn accepts_spotify_tracks_and_rejects_collections() {
    let song = SongLink::parse("https://open.spotify.com/intl-de/track/4cOdK2wGLETKBW3PvgPWqT?si=x").unwrap();
    assert_eq!(song.service(), Service::Spotify);
    assert!(!song.has_audio());
    assert_eq!(SongLink::parse("spotify:track:4cOdK2wGLETKBW3PvgPWqT").unwrap(), song);
    assert_eq!(
      SongLink::parse("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M"),
      Err(DownloadError::CollectionLink)
    );
    assert_eq!(
      SongLink::parse("https://www.youtube.com/playlist?list=PL123"),
      Err(DownloadError::CollectionLink)
    );
  }

  #[test]
  fn rejects_other_links() {
    for link in [
      "",
      "hello",
      "https://vimeo.com/123",
      "https://youtube.com/watch?v=short",
    ] {
      assert_eq!(SongLink::parse(link), Err(DownloadError::UnsupportedLink), "{link}");
    }
  }

  #[test]
  fn music_tags_win_over_the_video_title() {
    let facts = VideoFacts {
      title: "whatever (Official Video)".into(),
      channel: "Label".into(),
      artist: Some("21 Savage".into()),
      track: Some("a lot".into()),
      album: Some("i am > i was".into()),
    };
    assert_eq!(
      guess_tags(&facts),
      ("a lot".into(), "21 Savage".into(), Some("i am > i was".into()))
    );
  }

  #[test]
  fn video_titles_lose_their_noise() {
    let facts = |title: &str| VideoFacts {
      title: title.into(),
      channel: "SomeArtistVEVO".into(),
      ..VideoFacts::default()
    };
    assert_eq!(
      guess_tags(&facts("Daft Punk - One More Time (Official Video) [HD]")),
      ("One More Time".into(), "Daft Punk".into(), None)
    );
    assert_eq!(
      guess_tags(&facts("Artist – Song (feat. Other) (Lyrics)")).0,
      "Song (feat. Other)"
    );
    assert_eq!(
      guess_tags(&facts("Just A Song [Official Music Video]")),
      ("Just A Song".into(), "SomeArtist".into(), None)
    );
  }

  #[test]
  fn file_names_are_safe_and_unique() {
    let tags = SongTags::new(" Thunder? ", "AC/DC", None, None).unwrap();
    assert_eq!(tags.file_stem(), "AC_DC - Thunder_");
    let taken = ["a.mp3", "a (2).mp3"];
    assert_eq!(unique_file_name("a", "mp3", |n| taken.contains(&n)), "a (3).mp3");
    assert_eq!(unique_file_name("b", "mp3", |n| taken.contains(&n)), "b.mp3");
  }

  #[test]
  fn tags_need_title_and_artist() {
    assert_eq!(SongTags::new(" ", "x", None, None), Err(DownloadError::TitleEmpty));
    assert_eq!(SongTags::new("x", "", None, None), Err(DownloadError::ArtistEmpty));
    assert_eq!(SongTags::new("x", "y", Some("  "), None).unwrap().album(), None);
    assert_eq!(SongTags::new("x", "y", None, Some(99)), Err(DownloadError::YearInvalid));
    assert_eq!(SongTags::new("x", "y", None, Some(1993)).unwrap().year(), Some(1993));
  }

  #[test]
  fn sizes_follow_the_bitrate() {
    assert_eq!(AudioQuality::Mp3High.estimated_bytes(129, 60_000), 2_400_000);
    assert_eq!(AudioQuality::Original.estimated_bytes(128, 1_000), 16_000);
  }

  #[test]
  fn the_closest_length_wins() {
    let hit = |url: &str, duration_ms| Candidate {
      url: url.into(),
      duration_ms,
    };
    let best = closest_match(
      vec![hit("a", Some(200_000)), hit("b", None), hit("c", Some(181_000))],
      180_000,
    )
    .unwrap();
    assert_eq!(best.url, "c");
    assert!(matches!(closest_match(vec![], 1), Err(DownloadError::NoMatch)));
  }
}
