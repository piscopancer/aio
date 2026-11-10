use std::path::Path;
use std::thread;
use std::time::Duration;

use rodio::source::SineWave;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};

use crate::application::error::{AppError, AppResult};
use crate::application::ports::AudioOutput;
use crate::application::views::Track;
use crate::domain::playback::Volume;
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

/// Default sound device through rodio; a fresh `Player` per track, so dropping it silences the previous one.
pub struct RodioOutput {
  device: Option<MixerDeviceSink>,
  player: Option<Player>,
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
    Self { device, player: None }
  }
}

impl AudioOutput for RodioOutput {
  fn load(&mut self, track: &Track, volume: Volume, play: bool) -> AppResult<()> {
    self.player = None;
    let device = self
      .device
      .as_ref()
      .ok_or_else(|| AppError::Audio("no audio output device".into()))?;
    let player = Player::connect_new(device.mixer());
    player.set_volume(volume.gain());
    if !play {
      player.pause();
    }
    player.append(source(track)?);
    self.player = Some(player);
    Ok(())
  }

  fn unload(&mut self) {
    self.player = None;
  }

  fn play(&mut self) {
    if let Some(player) = &self.player {
      player.play();
    }
  }

  fn pause(&mut self) {
    if let Some(player) = &self.player {
      player.pause();
    }
  }

  fn seek(&mut self, position: Duration) -> AppResult<()> {
    let Some(player) = &self.player else { return Ok(()) };
    player
      .try_seek(position)
      .map_err(|error| AppError::Audio(format!("cannot seek: {error}")))
  }

  fn position(&self) -> Duration {
    self.player.as_ref().map(Player::get_pos).unwrap_or_default()
  }

  fn finished(&self) -> bool {
    self.player.as_ref().is_some_and(Player::empty)
  }

  fn set_volume(&mut self, volume: Volume) {
    if let Some(player) = &self.player {
      player.set_volume(volume.gain());
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
