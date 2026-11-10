use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rodio::source::SineWave;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};

use crate::application::error::{AppError, AppResult};
use crate::application::ports::AudioOutput;
use crate::application::views::Track;
use crate::domain::playback::Volume;
use crate::infrastructure::fader::{self, Course, FadeControl};
use crate::infrastructure::opus;

/// Path prefix of scenario tracks, which play a generated tone instead of a file.
pub const FIXTURE_PREFIX: &str = "fixture:";

/// Runs `run` on a dedicated audio thread with the default output device, which cannot leave that thread.
pub fn spawn(run: impl FnOnce(RodioOutput) + Send + 'static) {
  thread::Builder::new()
    .name("audio".into())
    .spawn(move || run(RodioOutput::open()))
    .expect("failed to spawn the audio thread");
}

/// One track on its own rodio `Player`, so dropping it silences that track alone.
struct Deck {
  player: Player,
  control: Arc<FadeControl>,
}

/// Default sound device through rodio; the loaded track plays on one deck while those it replaced fade out on theirs.
pub struct RodioOutput {
  device: Option<MixerDeviceSink>,
  current: Option<Deck>,
  /// Tracks replaced or stopped that are still fading out; each ends on its own once silent.
  leaving: Vec<Deck>,
}

impl RodioOutput {
  fn open() -> Self {
    let device = DeviceSinkBuilder::open_default_sink()
      .inspect_err(|error| eprintln!("no audio output: {error}"))
      .ok()
      .map(|mut device| {
        device.log_on_drop(false);
        device
      });
    Self {
      device,
      current: None,
      leaving: Vec::new(),
    }
  }

  /// Sends the loaded track off to fade out over `fade`, and lets go of those that have ended meanwhile.
  fn retire(&mut self, fade: Duration) {
    self.leaving.retain(|deck| !deck.player.empty());
    if let Some(deck) = self.current.take()
      && !fade.is_zero()
    {
      deck.control.steer(Course::End, fade);
      self.leaving.push(deck);
    }
  }

  fn decks(&self) -> impl Iterator<Item = &Deck> {
    self.current.iter().chain(&self.leaving)
  }
}

impl AudioOutput for RodioOutput {
  fn load(&mut self, track: &Track, volume: Volume, play: bool, fade: Duration) -> AppResult<()> {
    self.retire(fade);
    let device = self
      .device
      .as_ref()
      .ok_or_else(|| AppError::Audio("no audio output device".into()))?;
    let course = if play { Course::Rise } else { Course::Hold };
    let duration = Duration::from_millis(track.duration_ms.into());
    let (source, control) = fader::fade(source(track)?, duration, course, fade);
    let player = Player::connect_new(device.mixer());
    player.set_volume(volume.gain());
    player.append(source);
    self.current = Some(Deck { player, control });
    Ok(())
  }

  fn unload(&mut self, fade: Duration) {
    self.retire(fade);
  }

  fn cut_leaving(&mut self) {
    self.leaving.clear();
  }

  fn play(&mut self, fade: Duration) {
    if let Some(deck) = &self.current {
      deck.control.steer(Course::Rise, fade);
    }
  }

  fn pause(&mut self, fade: Duration) {
    if let Some(deck) = &self.current {
      deck.control.steer(Course::Hold, fade);
    }
    // a track still fading out goes silent no later than the paused one
    for deck in &self.leaving {
      if deck.control.length() > fade {
        deck.control.steer(Course::End, fade);
      }
    }
  }

  fn seek(&mut self, position: Duration) -> AppResult<()> {
    let Some(deck) = &self.current else { return Ok(()) };
    deck
      .player
      .try_seek(position)
      .map_err(|error| AppError::Audio(format!("cannot seek: {error}")))
  }

  fn position(&self) -> Duration {
    self
      .current
      .as_ref()
      .map(|deck| deck.control.position())
      .unwrap_or_default()
  }

  fn finished(&self) -> bool {
    self.current.as_ref().is_some_and(|deck| deck.player.empty())
  }

  fn set_volume(&mut self, volume: Volume) {
    for deck in self.decks() {
      deck.player.set_volume(volume.gain());
    }
  }

  fn fade_tail(&mut self, length: Duration) {
    if let Some(deck) = &self.current {
      deck.control.set_tail(length);
    }
  }
}

fn source(track: &Track) -> AppResult<Box<dyn Source + Send>> {
  if track.path.starts_with(FIXTURE_PREFIX) {
    // two octaves of semitones from A3, so neighbouring fixture tracks sound different
    let semitone = (track.id.0 % 24) as f32;
    let tone = SineWave::new(220.0 * 2f32.powf(semitone / 12.0))
      .amplify(0.15)
      .take_duration(Duration::from_millis(track.duration_ms.into()));
    return Ok(Box::new(tone));
  }
  opus::open(Path::new(&track.path)).map_err(|error| AppError::Audio(format!("{}: {error}", track.path)))
}
