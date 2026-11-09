use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use super::downloads::now;
use super::error::{AppError, AppResult};
use super::playback::PlayerHandle;
use super::ports::{LibraryQueries, LyricsFind, LyricsFinder, LyricsQuery, LyricsRecord, LyricsStore, SongFiles};
use super::views::{LyricLine, LyricWord, LyricsOrigin, LyricsView};
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
    let track = self
      .queries
      .tracks_by_ids(&[id])?
      .into_iter()
      .next()
      .ok_or(AppError::NotFound)?;
    // the song asked about is usually the one starting to play, so the one before it can now be written
    self.write_unwritten_later();
    let record = self
      .store
      .lyrics_record(id)?
      .filter(|record| same_recording(record.length_ms, track.duration_ms));

    if let Some(text) = self.songs.lyrics(Path::new(&track.path))? {
      let origin = match &record {
        Some(LyricsRecord {
          find: LyricsFind::Text { origin, .. },
          in_file: true,
          ..
        }) => *origin,
        _ => LyricsOrigin::File,
      };
      return Ok(view(&text, origin));
    }
    match record {
      Some(LyricsRecord {
        find: LyricsFind::Text { text, origin },
        ..
      }) => return Ok(view(&text, origin)),
      Some(LyricsRecord {
        find: LyricsFind::Instrumental,
        ..
      }) => return Ok(LyricsView::Instrumental),
      Some(LyricsRecord {
        find: LyricsFind::Missing,
        checked_at,
        ..
      }) if now() - checked_at < MISSING_RETRY_SECS => return Ok(LyricsView::Missing),
      _ => {}
    }

    let found = self.finder.find(&LyricsQuery {
      title: &track.title,
      // sites file a song under its main artist, the guests going into the title
      artist: track.artists.first().map(|artist| artist.name.as_str()),
      album: track.album.as_deref(),
      duration_ms: track.duration_ms,
    })?;
    let record = LyricsRecord {
      length_ms: found.as_ref().map_or(track.duration_ms, |found| found.length_ms),
      find: found.map_or(LyricsFind::Missing, |found| found.find),
      in_file: false,
      checked_at: now(),
    };
    self.store.save_lyrics_record(id, &record)?;
    Ok(match record.find {
      LyricsFind::Text { text, origin } => view(&text, origin),
      LyricsFind::Instrumental => LyricsView::Instrumental,
      LyricsFind::Missing => LyricsView::Missing,
    })
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

fn view(text: &str, origin: LyricsOrigin) -> LyricsView {
  match Lyrics::parse(text) {
    Some(Lyrics::Synced(lines)) => LyricsView::Synced {
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
