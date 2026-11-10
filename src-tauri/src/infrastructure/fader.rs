//! A source whose loudness glides between silence and full, so a song fades in, fades out, pauses and ends without a click.

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Sample, SampleRate, Source};

/// Where a faded song is heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Course {
  /// Up to full loudness.
  Rise,
  /// Down to silence, where it waits without moving on, as a paused song does.
  Hold,
  /// Down to silence, where it ends.
  End,
}

impl Course {
  fn from_u8(value: u8) -> Self {
    match value {
      0 => Self::Rise,
      1 => Self::Hold,
      _ => Self::End,
    }
  }
}

/// The handle the player steers a faded song with, read by the sound device as it plays.
pub struct FadeControl {
  course: AtomicU8,
  /// Seconds a fade from silence to full takes, as `f32` bits; zero switches at once.
  length: AtomicU32,
  /// Seconds over which the last moment of the song fades out, as `f32` bits; zero plays to the end at full.
  tail: AtomicU32,
  /// Seconds into the song, as `f64` bits.
  position: AtomicU64,
}

impl FadeControl {
  pub fn steer(&self, course: Course, length: Duration) {
    self.length.store(length.as_secs_f32().to_bits(), Ordering::Relaxed);
    self.course.store(course as u8, Ordering::Relaxed);
  }

  pub fn course(&self) -> Course {
    Course::from_u8(self.course.load(Ordering::Relaxed))
  }

  pub fn length(&self) -> Duration {
    Duration::from_secs_f32(f32::from_bits(self.length.load(Ordering::Relaxed)))
  }

  pub fn set_tail(&self, length: Duration) {
    self.tail.store(length.as_secs_f32().to_bits(), Ordering::Relaxed);
  }

  pub fn position(&self) -> Duration {
    Duration::from_secs_f64(f64::from_bits(self.position.load(Ordering::Relaxed)))
  }

  fn set_position(&self, seconds: f64) {
    self.position.store(seconds.to_bits(), Ordering::Relaxed);
  }
}

/// Wraps `inner`, a song of `duration`, starting silent and heading on `course` over `length`.
pub fn fade<S: Source>(inner: S, duration: Duration, course: Course, length: Duration) -> (Fader<S>, Arc<FadeControl>) {
  let control = Arc::new(FadeControl {
    course: AtomicU8::new(course as u8),
    length: AtomicU32::new(length.as_secs_f32().to_bits()),
    tail: AtomicU32::new(0),
    position: AtomicU64::new(0),
  });
  let fader = Fader {
    inner,
    control: control.clone(),
    duration: duration.as_secs_f64(),
    level: 0.0,
    gain: 0.0,
    elapsed: 0.0,
    channel: 0,
    silent: false,
  };
  (fader, control)
}

/// The faded song itself; the loudness changes once per frame, so the channels of a frame stay in step.
pub struct Fader<S> {
  inner: S,
  control: Arc<FadeControl>,
  duration: f64,
  /// How far between silence (0) and full (1) the fade is.
  level: f32,
  /// The factor the samples of the current frame are multiplied by.
  gain: f32,
  /// Seconds of the song played so far, counted by the frames taken from it.
  elapsed: f64,
  /// The place of the next sample within its frame.
  channel: u16,
  /// Whether the current frame is silence put in while the song holds, rather than a frame of the song.
  silent: bool,
}

impl<S: Source> Fader<S> {
  /// Moves the fade one frame on; `None` once the song has faded out for good.
  fn begin_frame(&mut self) -> Option<()> {
    let course = self.control.course();
    let rate = self.inner.sample_rate().get() as f32;
    let length = f32::from_bits(self.control.length.load(Ordering::Relaxed));
    let step = if length > 0.0 { 1.0 / (length * rate) } else { 1.0 };
    let level = match course {
      Course::Rise => self.level + step,
      Course::Hold | Course::End => self.level - step,
    };
    // snapped to the ends, so steps that add up a little off still arrive
    self.level = if level < f32::EPSILON {
      0.0
    } else if level > 1.0 - f32::EPSILON {
      1.0
    } else {
      level
    };
    self.silent = self.level == 0.0 && course != Course::Rise;
    if self.silent && course == Course::End {
      return None;
    }
    if self.silent {
      return Some(());
    }
    self.elapsed += 1.0 / f64::from(rate);
    self.control.set_position(self.elapsed);
    let tail = f64::from(f32::from_bits(self.control.tail.load(Ordering::Relaxed)));
    let closing = if tail > 0.0 {
      ((self.duration - self.elapsed) / tail).clamp(0.0, 1.0) as f32
    } else {
      1.0
    };
    // equal power: two songs crossing at the same pace keep the loudness of one
    self.gain = (self.level * closing * FRAC_PI_2).sin();
    Some(())
  }
}

impl<S: Source> Iterator for Fader<S> {
  type Item = Sample;

  fn next(&mut self) -> Option<Sample> {
    if self.channel == 0 {
      self.begin_frame()?;
    }
    self.channel = (self.channel + 1) % self.inner.channels().get();
    if self.silent {
      return Some(0.0);
    }
    self.inner.next().map(|sample| sample * self.gain)
  }
}

impl<S: Source> Source for Fader<S> {
  fn current_span_len(&self) -> Option<usize> {
    self.inner.current_span_len()
  }

  fn channels(&self) -> ChannelCount {
    self.inner.channels()
  }

  fn sample_rate(&self) -> SampleRate {
    self.inner.sample_rate()
  }

  fn total_duration(&self) -> Option<Duration> {
    self.inner.total_duration()
  }

  fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
    self.inner.try_seek(position)?;
    self.elapsed = position.as_secs_f64();
    self.channel = 0;
    self.control.set_position(self.elapsed);
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use std::num::NonZero;

  use rodio::buffer::SamplesBuffer;

  use super::*;

  /// One second of full-scale mono sound at 100 frames a second.
  fn second() -> SamplesBuffer {
    SamplesBuffer::new(NonZero::new(1).unwrap(), NonZero::new(100).unwrap(), vec![1.0; 100])
  }

  #[test]
  fn a_song_rises_from_silence_and_holds_without_moving_on() {
    let (mut fader, control) = fade(
      second(),
      Duration::from_secs(1),
      Course::Rise,
      Duration::from_millis(100),
    );
    let rising: Vec<Sample> = fader.by_ref().take(10).collect();
    assert!(rising.windows(2).all(|pair| pair[0] < pair[1]));
    assert!((rising[9] - 1.0).abs() < 1e-6);
    control.steer(Course::Hold, Duration::ZERO);
    assert_eq!(fader.by_ref().take(5).collect::<Vec<_>>(), [0.0; 5]);
    assert!(control.position().abs_diff(Duration::from_millis(100)) < Duration::from_micros(1));
    control.steer(Course::Rise, Duration::ZERO);
    assert!(fader.next().is_some_and(|sample| (sample - 1.0).abs() < 1e-6));
  }

  #[test]
  fn a_song_sent_off_ends_once_silent() {
    let (mut fader, control) = fade(second(), Duration::from_secs(1), Course::Rise, Duration::ZERO);
    fader.next();
    control.steer(Course::End, Duration::from_millis(50));
    assert_eq!(fader.count(), 4);
  }

  #[test]
  fn the_tail_fades_the_last_moment_out() {
    let (fader, control) = fade(second(), Duration::from_secs(1), Course::Rise, Duration::ZERO);
    control.set_tail(Duration::from_millis(200));
    let samples: Vec<Sample> = fader.collect();
    assert_eq!(samples.len(), 100);
    assert!((samples[79] - 1.0).abs() < 1e-6);
    assert!(samples[90] < 0.8 && samples[99] < 1e-3);
  }
}
