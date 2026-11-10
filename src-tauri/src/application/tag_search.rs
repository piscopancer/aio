use std::path::Path;
use std::sync::Arc;
use std::thread;

use serde::Deserialize;
use specta::Type;

use super::error::{AppError, AppResult, Missing};
use super::ports::{CoverImages, LibraryQueries, SongLookup, TagFinder, WebImages};
use super::views::{TagMatch, TagSearch, TagSource};
use crate::domain::ids::TrackId;
use crate::domain::tag_search::{answers_title, close_length};

/// What the edit dialog asks the databases about: the title and artists in its form as they are now, which may differ
/// from the file's.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagSearchQuery {
  pub title: String,
  pub artists: Vec<String>,
}

/// Finding the tags and covers of a song in online music databases, as variants the user picks from.
pub struct TagSearchService {
  /// Asked all at once; their finds are listed in this order.
  finders: Vec<Arc<dyn TagFinder>>,
  queries: Arc<dyn LibraryQueries>,
  images: Arc<dyn WebImages>,
  covers: Arc<dyn CoverImages>,
}

impl TagSearchService {
  pub fn new(
    finders: Vec<Arc<dyn TagFinder>>,
    queries: Arc<dyn LibraryQueries>,
    images: Arc<dyn WebImages>,
    covers: Arc<dyn CoverImages>,
  ) -> Self {
    Self {
      finders,
      queries,
      images,
      covers,
    }
  }

  /// The variants every database has for the song without repeats, those close to its length first, since the rest
  /// are more likely a remix or a live take than an edited file. Finds by title must answer the title asked; finds by
  /// sound need not, since they are found by the recording itself. A database that fails is named instead, and the
  /// others still answer. A blank title is looked up by the file name.
  pub fn search(&self, id: TrackId, query: &TagSearchQuery) -> AppResult<TagSearch> {
    let track = self
      .queries
      .tracks_by_ids(&[id])?
      .into_iter()
      .next()
      .ok_or(AppError::NotFound(Missing::Track(id)))?;
    let path = Path::new(&track.path);
    let stem = path
      .file_stem()
      .map(|stem| stem.to_string_lossy().into_owned())
      .unwrap_or_default();
    let title = Some(query.title.trim())
      .filter(|title| !title.is_empty())
      .unwrap_or(&stem);
    let artist = query
      .artists
      .first()
      .map(|artist| artist.trim())
      .filter(|artist| !artist.is_empty());
    let song = SongLookup {
      title,
      artist,
      path,
      duration_ms: track.duration_ms,
    };

    let answers: Vec<_> = thread::scope(|scope| {
      let asks: Vec<_> = self
        .finders
        .iter()
        .map(|finder| (finder.source(), scope.spawn(|| finder.find(&song))))
        .collect();
      asks
        .into_iter()
        .map(|(source, ask)| {
          (
            source,
            ask.join().unwrap_or_else(|_| Err(AppError::Fetch("crashed".into()))),
          )
        })
        .collect()
    });

    let mut search = TagSearch {
      matches: Vec::new(),
      failed: Vec::new(),
    };
    for (source, answer) in answers {
      match answer {
        Ok(found) => {
          for found in found {
            let relevant = source == TagSource::AcoustId || answers_title(&found.title, title);
            if relevant && !search.matches.iter().any(|known| same_tags(known, &found)) {
              search.matches.push(found);
            }
          }
        }
        Err(error) => {
          eprintln!("tag search on {source:?} failed: {error}");
          search.failed.push(source);
        }
      }
    }
    search
      .matches
      .sort_by_key(|found| !close_length(found.duration_ms, track.duration_ms));
    Ok(search)
  }

  /// Downloads a found cover and stores it as a square, returning the stored copy's path for the edit form.
  pub fn import_cover(&self, url: &str) -> AppResult<String> {
    let image = self.images.fetch(url)?;
    self.covers.import_bytes(&image)
  }
}

/// Whether two finds offer the very same tags, as two stores of one database often do; the first one found stays.
fn same_tags(a: &TagMatch, b: &TagMatch) -> bool {
  a.title == b.title
    && a.artists == b.artists
    && a.album == b.album
    && a.year == b.year
    && a.track_no == b.track_no
    && a.disc_no == b.disc_no
    && a.genre == b.genre
}
