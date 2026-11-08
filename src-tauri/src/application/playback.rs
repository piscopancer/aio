use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use super::error::AppResult;
use super::ports::{AudioOutput, LibraryQueries, Notifier, PlaybackSettings, SettingsStore};
use super::views::{PlayerState, QueueItem, Track};
use crate::domain::ids::TrackId;
use crate::domain::playback::{
  Advance, PlaybackContext, PlaybackError, Previous, Queue, QueueEntry, QueueKey, Removal, RepeatMode, Volume,
};

/// How often the player checks for the end of a track and reports the position.
const TICK: Duration = Duration::from_millis(250);

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
  SetVolume(Volume),
  SetShuffle(bool),
  SetRepeat(RepeatMode),
  Enqueue(Vec<Track>),
  Remove(QueueKey),
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

  /// Queues the tracks of a context and starts at `start`, or at the first track when it is not given.
  pub fn play(&self, queries: &dyn LibraryQueries, context: PlaybackContext, start: Option<TrackId>) -> AppResult<()> {
    let tracks = queries.context_tracks(context)?;
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
  notifier: Arc<dyn Notifier>,
  queue: Queue<Track>,
  volume: Volume,
  loaded: bool,
  playing: bool,
}

impl<O: AudioOutput> PlaybackService<O> {
  pub fn new(output: O, inbox: PlayerInbox, settings: Arc<dyn SettingsStore>, notifier: Arc<dyn Notifier>) -> Self {
    let saved = settings.playback_settings();
    Self {
      output,
      inbox,
      settings,
      notifier,
      queue: Queue::new(saved.shuffle, saved.repeat),
      volume: saved.volume,
      loaded: false,
      playing: false,
    }
  }

  pub fn run(mut self) {
    loop {
      match self.inbox.commands.recv_timeout(TICK) {
        Ok(command) => {
          if let Err(error) = self.handle(command) {
            self.notifier.playback_failed(&error);
          }
          self.publish();
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
      } else {
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
          self.load_current(true)?;
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
        self.output.set_volume(volume);
        self.save_settings()
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
    self.loaded = false;
    self.playing = false;
    let Some(track) = self.queue.current().map(|entry| entry.item().clone()) else {
      self.output.unload();
      return Err(PlaybackError::NothingToPlay.into());
    };
    self.output.load(&track, self.volume, play)?;
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

  fn save_settings(&self) -> AppResult<()> {
    self.settings.save_playback_settings(PlaybackSettings {
      volume: self.volume,
      shuffle: self.queue.shuffle(),
      repeat: self.queue.repeat(),
    })
  }

  fn position_ms(&self) -> u32 {
    if self.loaded {
      u32::try_from(self.output.position().as_millis()).unwrap_or(u32::MAX)
    } else {
      0
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
