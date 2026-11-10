use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use super::downloads::now;
use super::error::{AppError, AppResult, Missing};
use super::playback::PlayerHandle;
use super::ports::{
  LibraryQueries, LyricsFind, LyricsFinder, LyricsQuery, LyricsRecord, LyricsState, LyricsStore, SongFiles,
};
use super::views::{LyricLine, LyricWord, LyricsOrigin, LyricsView, Track};
use crate::domain::ids::TrackId;
use crate::domain::lyrics::{Lyrics, same_recording};

/// How long a song LRCLIB had no lyrics for is not asked about again.
const MISSING_RETRY_SECS: i64 = 24 * 60 * 60;

/// Finding the lyrics of songs: in their files first, then on LRCLIB, whose finds are written into the files once
/// they are not playing.
pub struct LyricsService {
  songs: Arc<dyn SongFiles>,
  finder: Arc<dyn LyricsFinder>,
  store: Arc<dyn LyricsStore>,
  queries: Arc<dyn LibraryQueries>,
  player: Arc<PlayerHandle>,
  /// Held while finds are written into files, so two passes never rewrite the same file at once.
  writing: Mutex<()>,
}

impl LyricsService {
  pub fn new(
    songs: Arc<dyn SongFiles>,
    finder: Arc<dyn LyricsFinder>,
    store: Arc<dyn LyricsStore>,
    queries: Arc<dyn LibraryQueries>,
    player: Arc<PlayerHandle>,
  ) -> Self {
    Self {
      songs,
      finder,
      store,
      queries,
      player,
      writing: Mutex::new(()),
    }
  }

  pub fn lyrics(self: &Arc<Self>, id: TrackId) -> AppResult<LyricsView> {
    let track = self.track(id)?;
    // the song asked about is usually the one starting to play, so the one before it can now be written
    self.write_unwritten_later();
    let record = self
      .store
      .lyrics_record(id)?
      .filter(|record| same_recording(record.length_ms, track.duration_ms));

    if let Some(text) = self.songs.lyrics(Path::new(&track.path))? {
      let origin = match &record {
        Some(LyricsRecord {
          state: LyricsState::Written { origin },
          ..
        }) => *origin,
        _ => LyricsOrigin::File,
      };
      return Ok(view(&text, origin));
    }
    // lyrics written into a file that lost them since are looked up again
    match record {
      Some(LyricsRecord {
        state: LyricsState::Unwritten { text, origin },
        ..
      }) => return Ok(view(&text, origin)),
      Some(LyricsRecord {
        state: LyricsState::Instrumental,
        ..
      }) => return Ok(LyricsView::Instrumental),
      Some(LyricsRecord {
        state: LyricsState::Missing,
        checked_at,
        ..
      }) if now() - checked_at < MISSING_RETRY_SECS => return Ok(LyricsView::Missing),
      _ => {}
    }

    let found = self.finder.find(&query(&track))?;
    let length_ms = found.as_ref().map_or(track.duration_ms, |found| found.length_ms);
    let find = found.map_or(LyricsFind::Missing, |found| found.find);
    let record = LyricsRecord {
      state: find.clone().into(),
      length_ms,
      checked_at: now(),
    };
    self.store.save_lyrics_record(id, &record)?;
    Ok(match find {
      LyricsFind::Text { text, origin } => view(&text, origin),
      LyricsFind::Instrumental => LyricsView::Instrumental,
      LyricsFind::Missing => LyricsView::Missing,
    })
  }

  /// What LRCLIB has for a song whose file holds lyrics of its own, when its words or timing differ from the file's.
  pub fn lrclib_lyrics(&self, id: TrackId) -> AppResult<Option<LyricsView>> {
    let track = self.track(id)?;
    let Some(own) = self.songs.lyrics(Path::new(&track.path))? else {
      return Ok(None);
    };
    let found = self.finder.find(&query(&track))?;
    Ok(match found.map(|found| found.find) {
      Some(LyricsFind::Text { text, origin }) if Lyrics::parse(&text) != Lyrics::parse(&own) => {
        Some(view(&text, origin))
      }
      _ => None,
    })
  }

  /// Writes `text` into the song's file over the lyrics it holds, letting go of the file meanwhile when it plays.
  pub fn save(&self, id: TrackId, text: &str, origin: LyricsOrigin) -> AppResult<()> {
    let track = self.track(id)?;
    let _writing = self.writing.lock().unwrap_or_else(PoisonError::into_inner);
    self.player.while_released(vec![track.path.clone()], || {
      let written = self.songs.write_lyrics(Path::new(&track.path), text);
      (written, self.queries.tracks_by_ids(&[id]).unwrap_or_default())
    })?;
    // remembered as written, so the panel credits where the words came from
    self.store.save_lyrics_record(
      id,
      &LyricsRecord {
        state: LyricsState::Written { origin },
        length_ms: track.duration_ms,
        checked_at: now(),
      },
    )
  }

  fn track(&self, id: TrackId) -> AppResult<Track> {
    self
      .queries
      .tracks_by_ids(&[id])?
      .into_iter()
      .next()
      .ok_or(AppError::NotFound(Missing::Track(id)))
  }

  pub fn write_unwritten_later(self: &Arc<Self>) {
    let service = self.clone();
    thread::spawn(move || {
      if let Err(error) = service.write_unwritten() {
        eprintln!("writing lyrics into files failed: {error}");
      }
    });
  }

  /// Writes found lyrics into their song files, except the file playing now, which waits for the next pass.
  fn write_unwritten(&self) -> AppResult<()> {
    let _writing = self.writing.lock().unwrap_or_else(PoisonError::into_inner);
    let state = self.player.state();
    let playing = state
      .index
      .and_then(|index| state.queue.get(index as usize))
      .map(|item| item.track.id);
    for song in self.store.unwritten_lyrics()? {
      if Some(song.track) == playing {
        continue;
      }
      match self.songs.write_lyrics(Path::new(&song.path), &song.text) {
        Ok(()) => self.store.mark_lyrics_written(song.track)?,
        Err(error) => eprintln!("cannot write lyrics into {}: {error}", song.path),
      }
    }
    Ok(())
  }
}

/// What the song is looked up on LRCLIB by.
fn query(track: &Track) -> LyricsQuery<'_> {
  LyricsQuery {
    title: &track.title,
    // sites file a song under its main artist, the guests going into the title
    artist: track.artists.first().map(|artist| artist.name.as_str()),
    album: track.album.as_deref(),
    duration_ms: track.duration_ms,
  }
}

fn view(text: &str, origin: LyricsOrigin) -> LyricsView {
  match Lyrics::parse(text) {
    Some(Lyrics::Synced(lines)) => LyricsView::Synced {
      text: text.to_owned(),
      lines: lines
        .into_iter()
        .map(|line| LyricLine {
          start_ms: line.start_ms,
          text: line.text,
          words: line
            .words
            .into_iter()
            .map(|word| LyricWord {
              start_ms: word.start_ms,
              text: word.text,
            })
            .collect(),
        })
        .collect(),
      origin,
    },
    Some(Lyrics::Plain(text)) => LyricsView::Plain { text, origin },
    None => LyricsView::Missing,
  }
}
