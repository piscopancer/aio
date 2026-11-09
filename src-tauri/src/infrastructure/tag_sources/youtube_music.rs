//! YouTube Music, asked through the search its own web player uses, which needs no key. Its songs are releases with an
//! album and a cover; when none of them is the song asked, its videos are asked too, since many songs exist there only
//! as uploads named `Artist - Title`.

use serde::Serialize;
use serde_json::Value;

use super::{RESULTS, ReadJson, agent};
use crate::application::error::AppResult;
use crate::application::ports::{SongLookup, TagFinder};
use crate::application::views::{FoundCover, TagMatch, TagSource};
use crate::domain::tag_search::{answers_title, credits, split_credit, upload_credit};
use crate::infrastructure::tools::fetch_error;

const SEARCH: &str = "https://music.youtube.com/youtubei/v1/search?prettyPrint=false";

/// The web player the search answers as; an outdated version is still served.
const CLIENT: (&str, &str) = ("WEB_REMIX", "1.20241106.01.00");

/// The web player's search filters, which keep only songs or only videos.
const SONGS: &str = "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D";
const VIDEOS: &str = "EgWKAQIQAWoKEAkQChAFEAMQBA%3D%3D";

/// What separates the parts of a result's second line, such as `Artist • Album • 3:15`.
const SEPARATOR: &str = " • ";

/// The kinds of page a part of a result's second line leads to.
const ARTIST_PAGE: &str = "MUSIC_PAGE_TYPE_ARTIST";
const ALBUM_PAGE: &str = "MUSIC_PAGE_TYPE_ALBUM";

#[derive(Serialize)]
struct Query<'a> {
  context: Context,
  query: &'a str,
  params: &'a str,
}

#[derive(Serialize)]
struct Context {
  client: Client,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Client {
  client_name: &'static str,
  client_version: &'static str,
  hl: &'static str,
}

/// A piece of a result's line, with the kind of page it leads to when it is a link.
struct Run<'a> {
  text: &'a str,
  page: Option<&'a str>,
}

/// The results of a search answer, each a `musicResponsiveListItemRenderer`.
fn items(answer: &Value) -> impl Iterator<Item = &Value> {
  answer
    .pointer("/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(|section| section.pointer("/musicShelfRenderer/contents")?.as_array())
    .flatten()
    .filter_map(|item| item.get("musicResponsiveListItemRenderer"))
}

/// The pieces of a result's `column`th line: the title, then the line of credits.
fn runs(item: &Value, column: usize) -> Vec<Run<'_>> {
  item
    .pointer(&format!(
      "/flexColumns/{column}/musicResponsiveListItemFlexColumnRenderer/text/runs"
    ))
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(|run| {
      Some(Run {
        text: run.get("text")?.as_str()?,
        page: run
          .pointer("/navigationEndpoint/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType")
          .and_then(Value::as_str),
      })
    })
    .collect()
}

/// The parts of a result's line between its separators.
fn parts<'a>(runs: &'a [Run<'a>]) -> Vec<&'a [Run<'a>]> {
  runs.split(|run| run.text == SEPARATOR).collect()
}

fn text(runs: &[Run<'_>]) -> String {
  runs.iter().map(|run| run.text).collect()
}

/// The length a clock such as `5:53` or `1:02:07` shows.
fn clock_ms(clock: &str) -> Option<u32> {
  clock
    .split(':')
    .try_fold(0u32, |seconds, part| Some(seconds * 60 + part.parse::<u32>().ok()?))
    .filter(|_| clock.contains(':'))
    .map(|seconds| seconds * 1000)
}

/// A found song, which names its artists, album and length, with its album cover at any size.
fn song(item: &Value) -> Option<TagMatch> {
  let title = text(&runs(item, 0));
  let credits = runs(item, 1);
  let parts = parts(&credits);
  let thumbnail = item
    .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")?
    .as_array()?
    .last()?
    .get("url")?
    .as_str()?;
  // the address ends in the size it is served at, such as `=w120-h120-l90-rj`
  let (base, _) = thumbnail.rsplit_once('=')?;
  Some(TagMatch {
    source: TagSource::YoutubeMusic,
    artists: split_credit(&text(parts.first()?)),
    album: credits
      .iter()
      .find(|run| run.page == Some(ALBUM_PAGE))
      .map(|run| run.text.to_owned()),
    year: None,
    track_no: None,
    disc_no: None,
    genre: None,
    duration_ms: parts.last().and_then(|part| clock_ms(&text(part))),
    cover: Some(FoundCover {
      thumb: format!("{base}=w250-h250-l90-rj"),
      full: format!("{base}=w1200-h1200-l90-rj"),
    }),
    title,
  })
}

/// A found video: its artist and title come from its name when it is named `Artist - Title`, or else the channel is
/// the artist when it is an artist's own; its picture is the video's.
fn video(item: &Value) -> Option<TagMatch> {
  let name = text(&runs(item, 0));
  let credits = runs(item, 1);
  let parts = parts(&credits);
  let channel = parts.first()?;
  let (artists, title) = match upload_credit(&name) {
    Some((artist, title)) => (split_credit(&artist), title),
    None if channel.iter().any(|run| run.page == Some(ARTIST_PAGE)) => (vec![text(channel)], name),
    None => (Vec::new(), name),
  };
  let id = item.pointer("/playlistItemData/videoId")?.as_str()?;
  let picture = format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg");
  Some(TagMatch {
    source: TagSource::YoutubeMusic,
    title,
    artists,
    album: None,
    year: None,
    track_no: None,
    disc_no: None,
    genre: None,
    duration_ms: parts.last().and_then(|part| clock_ms(&text(part))),
    cover: Some(FoundCover {
      thumb: picture.clone(),
      full: picture,
    }),
  })
}

pub struct YoutubeMusic {
  http: ureq::Agent,
}

impl YoutubeMusic {
  pub fn new() -> Self {
    Self { http: agent() }
  }

  fn search(&self, words: &str, filter: &str, read: fn(&Value) -> Option<TagMatch>) -> AppResult<Vec<TagMatch>> {
    let (client_name, client_version) = CLIENT;
    let query = Query {
      context: Context {
        client: Client {
          client_name,
          client_version,
          hl: "en",
        },
      },
      query: words,
      params: filter,
    };
    let answer: Value = self
      .http
      .post(SEARCH)
      .header("Content-Type", "application/json")
      .header("Origin", "https://music.youtube.com")
      .send(serde_json::to_string(&query).map_err(fetch_error)?)
      .map_err(fetch_error)?
      .body_mut()
      .json()?;
    Ok(items(&answer).filter_map(read).take(RESULTS).collect())
  }
}

impl TagFinder for YoutubeMusic {
  fn source(&self) -> TagSource {
    TagSource::YoutubeMusic
  }

  fn find(&self, song_asked: &SongLookup<'_>) -> AppResult<Vec<TagMatch>> {
    let words = song_asked.words();
    let mut found = self.search(&words, SONGS, song)?;
    let answered = found.iter().any(|found| {
      answers_title(&found.title, song_asked.title)
        && song_asked.artist.is_none_or(|artist| credits(&found.artists, artist))
    });
    if !answered {
      found.extend(self.search(&words, VIDEOS, video)?);
    }
    Ok(found)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_clock_turns_into_milliseconds() {
    assert_eq!(clock_ms("5:53"), Some(353_000));
    assert_eq!(clock_ms("1:02:07"), Some(3_727_000));
    assert_eq!(clock_ms("36K plays"), None);
    assert_eq!(clock_ms("12"), None);
  }
}
