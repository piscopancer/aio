use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use super::error::AppResult;
use super::ports::{AudioOutput, LibraryQueries, Notifier, PlaybackSettings, PlayerSession, SettingsStore};
use super::views::{PlayerState, QueueItem, Track};
use crate::domain::ids::TrackId;
use crate::domain::playback::{
  Advance, PlaybackContext, PlaybackError, Previous, Queue, QueueEntry, QueueKey, QueueSnapshot, Removal, RepeatMode,
  Volume, resume_point,
};

/// How often the player checks for the end of a track and reports the position.
const TICK: Duration = Duration::from_millis(250);

/// How often the position of a playing song is saved, so that a crash loses no more than this of it; the queue is
/// saved whenever it changes.
const SAVE_POSITION_EVERY: Duration = Duration::from_secs(5);

pub enum PlayerCommand {
  Start {
    context: PlaybackContext,
    tracks: Vec<Track>,
    start: usize,
  },
  Jump(QueueKey),
  Toggle,
  Next,
  Previous,
  Seek(u32),
  /// Sets the level and lets the sound back in when it was muted.
  SetVolume(Volume),
  /// Silences the player or lets it play at its level again, which stays as it was.
  SetMuted(bool),
  SetShuffle(bool),
  SetRepeat(RepeatMode),
  Enqueue(Vec<Track>),
  Remove(QueueKey),
  /// Lets go of the playing file when it is one of `paths`, remembering where it was, so the file can be rewritten;
  /// `done` hears back once nothing holds it.
  Release {
    paths: Vec<String>,
    done: Sender<()>,
  },
  /// Swaps in the newer versions of queued tracks and picks a released file up again where it was.
  Refresh(Vec<Track>),
}

/// Where a released track was, so it resumes there.
#[derive(Clone, Copy)]
struct Suspended {
  position: Duration,
  playing: bool,
}

/// Thread-safe handle the interface uses to drive the player and read its last published state.
pub struct PlayerHandle {
  commands: Sender<PlayerCommand>,
  state: Arc<Mutex<PlayerState>>,
}

/// The receiving end of a `PlayerHandle`, consumed by the `PlaybackService` on the audio thread.
pub struct PlayerInbox {
  commands: Receiver<PlayerCommand>,
  state: Arc<Mutex<PlayerState>>,
}

pub fn player_channel(settings: PlaybackSettings) -> (PlayerHandle, PlayerInbox) {
  let (sender, receiver) = mpsc::channel();
  let state = Arc::new(Mutex::new(PlayerState {
    context: None,
    queue: Vec::new(),
    index: None,
    is_playing: false,
    position_ms: 0,
    volume: settings.volume.percent(),
    muted: false,
    shuffle: settings.shuffle,
    repeat: settings.repeat,
  }));
  (
    PlayerHandle {
      commands: sender,
      state: state.clone(),
    },
    PlayerInbox {
      commands: receiver,
      state,
    },
  )
}

impl PlayerHandle {
  pub fn send(&self, command: PlayerCommand) {
    // the player only stops when the app exits, so a failed send has nobody left to tell
    let _ = self.commands.send(command);
  }

  pub fn state(&self) -> PlayerState {
    lock(&self.state).clone()
  }

  /// Runs `rewrite` on files while the player holds none of `paths`, then hands it the tracks as they are after.
  pub fn while_released<T>(&self, paths: Vec<String>, rewrite: impl FnOnce() -> (T, Vec<Track>)) -> T {
    let (done, released) = mpsc::channel();
    self.send(PlayerCommand::Release { paths, done });
    // a player that is gone holds nothing either
    let _ = released.recv();
    let (result, tracks) = rewrite();
    self.send(PlayerCommand::Refresh(tracks));
    result
  }

  /// Queues the tracks of a context and starts at `start`, or at the first track when it is not given; `order` lists the tracks in the order the view shows them when it is sorted, and the queue follows it.
  pub fn play(
    &self,
    queries: &dyn LibraryQueries,
    context: PlaybackContext,
    start: Option<TrackId>,
    order: Option<&[TrackId]>,
  ) -> AppResult<()> {
    let mut tracks = queries.context_tracks(context)?;
    if let Some(order) = order {
      arrange(&mut tracks, order);
    }
    if tracks.is_empty() {
      return Err(PlaybackError::NothingToPlay.into());
    }
    let start = start
      .and_then(|id| tracks.iter().position(|track| track.id == id))
      .unwrap_or(0);
    self.send(PlayerCommand::Start { context, tracks, start });
    Ok(())
  }

  pub fn enqueue(&self, queries: &dyn LibraryQueries, ids: &[TrackId]) -> AppResult<()> {
    let tracks = queries.tracks_by_ids(ids)?;
    self.send(PlayerCommand::Enqueue(tracks));
    Ok(())
  }
}

/// Runs the queue aggregate against an audio output; lives on the audio thread for the whole app lifetime.
pub struct PlaybackService<O: AudioOutput> {
  output: O,
  inbox: PlayerInbox,
  settings: Arc<dyn SettingsStore>,
  queries: Arc<dyn LibraryQueries>,
  notifier: Arc<dyn Notifier>,
  queue: Queue<Track>,
  volume: Volume,
  /// The sound is off while the level stays, so unmuting goes back to it; it lasts for the session only.
  muted: bool,
  loaded: bool,
  playing: bool,
  suspended: Option<Suspended>,
  /// Where the current song resumes when it is first played, after the session was restored at startup.
  resume_at: Option<Duration>,
  /// The queue and position saved last, so that only what changed is written again.
  saved_queue: Option<QueueSnapshot<TrackId>>,
  saved_position_ms: u32,
  saved_at: Instant,
}

impl<O: AudioOutput> PlaybackService<O> {
  pub fn new(
    output: O,
    inbox: PlayerInbox,
    settings: Arc<dyn SettingsStore>,
    queries: Arc<dyn LibraryQueries>,
    notifier: Arc<dyn Notifier>,
  ) -> Self {
    let saved = settings.playback_settings();
    Self {
      output,
      inbox,
      settings,
      queries,
      notifier,
      queue: Queue::new(saved.shuffle, saved.repeat),
      volume: saved.volume,
      muted: false,
      loaded: false,
      playing: false,
      suspended: None,
      resume_at: None,
      saved_queue: None,
      saved_position_ms: 0,
      saved_at: Instant::now(),
    }
  }

  pub fn run(mut self) {
    self.restore();
    self.publish();
    loop {
      match self.inbox.commands.recv_timeout(TICK) {
        Ok(command) => {
          if let Err(error) = self.handle(command) {
            self.notifier.playback_failed(&error);
          }
          self.publish();
          self.save_session();
        }
        Err(RecvTimeoutError::Timeout) => {}
        Err(RecvTimeoutError::Disconnected) => break,
      }
      if !self.playing {
        continue;
      }
      if self.output.finished() {
        if let Err(error) = self.next(Advance::TrackEnded) {
          self.notifier.playback_failed(&error);
        }
        self.publish();
        self.save_session();
      } else {
        if self.saved_at.elapsed() >= SAVE_POSITION_EVERY {
          self.save_session();
        }
        let position_ms = self.position_ms();
        lock(&self.inbox.state).position_ms = position_ms;
        self.notifier.playback_progress(position_ms);
      }
    }
  }

  fn handle(&mut self, command: PlayerCommand) -> AppResult<()> {
    match command {
      PlayerCommand::Start { context, tracks, start } => {
        self.queue.start(context, tracks, start, &mut rand::rng())?;
        self.load_current(true)
      }
      PlayerCommand::Jump(key) => {
        self.queue.jump(key)?;
        self.load_current(true)
      }
      PlayerCommand::Toggle => {
        if !self.loaded {
          match self.resume_at.take() {
            Some(position) => {
              self.load_current(false)?;
              self.seek(position)?;
              self.output.play();
              self.playing = true;
            }
            None => self.load_current(true)?,
          }
        } else if self.playing {
          self.output.pause();
          self.playing = false;
        } else {
          self.output.play();
          self.playing = true;
        }
        Ok(())
      }
      PlayerCommand::Next => self.next(Advance::Skipped),
      PlayerCommand::Previous => match self.queue.go_back(self.output.position()) {
        Previous::Restart if self.loaded => self.seek(Duration::ZERO),
        Previous::Restart => Ok(()),
        Previous::Go(_) => self.load_current(true),
      },
      PlayerCommand::Seek(position_ms) => {
        if !self.loaded {
          self.load_current(false)?;
        }
        self.seek(Duration::from_millis(position_ms.into()))
      }
      PlayerCommand::SetVolume(volume) => {
        self.volume = volume;
        self.muted = false;
        self.output.set_volume(self.audible());
        self.save_settings()
      }
      PlayerCommand::SetMuted(muted) => {
        self.muted = muted;
        self.output.set_volume(self.audible());
        Ok(())
      }
      PlayerCommand::SetShuffle(shuffle) => {
        self.queue.set_shuffle(shuffle, &mut rand::rng());
        self.save_settings()
      }
      PlayerCommand::SetRepeat(repeat) => {
        self.queue.set_repeat(repeat);
        self.save_settings()
      }
      PlayerCommand::Enqueue(tracks) => {
        self.queue.enqueue(tracks);
        Ok(())
      }
      PlayerCommand::Remove(key) => match self.queue.remove(key)? {
        Removal::Other => Ok(()),
        Removal::Current { has_next: true } => self.load_current(self.playing),
        Removal::Current { has_next: false } => {
          self.stop();
          Ok(())
        }
      },
      PlayerCommand::Release { paths, done } => {
        let held = self.loaded
          && self
            .queue
            .current()
            .is_some_and(|entry| paths.contains(&entry.item().path));
        if held {
          self.suspended = Some(Suspended {
            position: self.output.position(),
            playing: self.playing,
          });
          self.stop();
        }
        let _ = done.send(());
        Ok(())
      }
      PlayerCommand::Refresh(tracks) => {
        self
          .queue
          .refresh(|item| tracks.iter().find(|track| track.id == item.id).cloned());
        let Some(suspended) = self.suspended.take() else {
          return Ok(());
        };
        self.load_current(false)?;
        self.seek(suspended.position)?;
        if suspended.playing {
          self.output.play();
          self.playing = true;
        }
        Ok(())
      }
    }
  }

  fn next(&mut self, reason: Advance) -> AppResult<()> {
    if self.queue.advance(reason) {
      self.load_current(true)
    } else {
      self.stop();
      Ok(())
    }
  }

  fn load_current(&mut self, play: bool) -> AppResult<()> {
    // another song, or the same one from wherever it is asked for, so the restored place no longer applies
    self.resume_at = None;
    self.loaded = false;
    self.playing = false;
    let Some(track) = self.queue.current().map(|entry| entry.item().clone()) else {
      self.output.unload();
      return Err(PlaybackError::NothingToPlay.into());
    };
    self.output.load(&track, self.audible(), play)?;
    self.loaded = true;
    self.playing = play;
    Ok(())
  }

  fn seek(&mut self, position: Duration) -> AppResult<()> {
    self.output.seek(position)?;
    self.notifier.playback_progress(self.position_ms());
    Ok(())
  }

  fn stop(&mut self) {
    self.output.unload();
    self.loaded = false;
    self.playing = false;
  }

  /// The level the output plays at: silence while muted, the set level otherwise.
  fn audible(&self) -> Volume {
    if self.muted { Volume::SILENT } else { self.volume }
  }

  fn save_settings(&self) -> AppResult<()> {
    self.settings.save_playback_settings(PlaybackSettings {
      volume: self.volume,
      shuffle: self.queue.shuffle(),
      repeat: self.queue.repeat(),
    })
  }

  fn position_ms(&self) -> u32 {
    let position = if self.loaded {
      self.output.position()
    } else {
      self.resume_at.unwrap_or_default()
    };
    u32::try_from(position.as_millis()).unwrap_or(u32::MAX)
  }

  /// Takes up the session of the last run, paused, with the songs that left the library dropped from its queue; the
  /// current song resumes a little before where it stopped, or from its start when it is a different one.
  fn restore(&mut self) {
    let Some(session) = self.settings.player_session() else {
      return;
    };
    let mut ids = session.queue.items.clone();
    ids.sort_unstable();
    ids.dedup();
    let tracks: HashMap<TrackId, Track> = match self.queries.tracks_by_ids(&ids) {
      Ok(tracks) => tracks.into_iter().map(|track| (track.id, track)).collect(),
      Err(error) => {
        eprintln!("restoring the player failed: {error}");
        return;
      }
    };
    let (queue, current_kept) = session.queue.resolve(|id| tracks.get(&id).cloned());
    self.queue.restore(queue);
    if self.queue.current().is_some() {
      let stopped = Duration::from_millis(session.position_ms.into());
      self.resume_at = Some(if current_kept {
        resume_point(stopped)
      } else {
        Duration::ZERO
      });
    }
  }

  /// Saves the queue when it changed, or else the position when it moved; a failure is only logged, since playing
  /// goes on all the same.
  fn save_session(&mut self) {
    let queue = self.queue.snapshot(|track| track.id);
    let position_ms = self.position_ms();
    let saved = if self.saved_queue.as_ref() != Some(&queue) {
      let session = PlayerSession { queue, position_ms };
      let saved = self.settings.save_player_session(&session);
      self.saved_queue = Some(session.queue);
      saved
    } else if position_ms != self.saved_position_ms {
      self.settings.save_player_position(position_ms)
    } else {
      return;
    };
    self.saved_position_ms = position_ms;
    self.saved_at = Instant::now();
    if let Err(error) = saved {
      eprintln!("saving the player session failed: {error}");
    }
  }

  fn publish(&self) {
    let state = PlayerState {
      context: self.queue.context(),
      queue: self.queue.entries().iter().map(queue_item).collect(),
      index: self.queue.current_index().and_then(|index| u32::try_from(index).ok()),
      is_playing: self.playing,
      position_ms: self.position_ms(),
      volume: self.volume.percent(),
      muted: self.muted,
      shuffle: self.queue.shuffle(),
      repeat: self.queue.repeat(),
    };
    self.notifier.player_changed(&state);
    *lock(&self.inbox.state) = state;
  }
}

fn queue_item(entry: &QueueEntry<Track>) -> QueueItem {
  QueueItem {
    key: entry.key(),
    track: entry.item().clone(),
  }
}

fn lock(state: &Mutex<PlayerState>) -> MutexGuard<'_, PlayerState> {
  // a poisoned lock only means a reader panicked; the snapshot itself is always whole
  state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Puts the tracks in the order of `order`; tracks it leaves out, as when the view is a moment behind the library, keep their own order after the rest.
fn arrange(tracks: &mut [Track], order: &[TrackId]) {
  let rank: HashMap<TrackId, usize> = order.iter().enumerate().map(|(index, id)| (*id, index)).collect();
  tracks.sort_by_key(|track| rank.get(&track.id).copied().unwrap_or(usize::MAX));
}
