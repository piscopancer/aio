//! Ogg Opus playback, which rodio's Symphonia decoders lack: the pure-Rust `opus-pure` reads the stream and decodes it,
//! so no C library has to be built for it.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use opus_pure::ogg::GRANULE_RATE;
use opus_pure::{MAX_PACKET_SAMPLES, OggOpusReader, OggPacket, OpusDecoder, OpusHead, Trim};
use rodio::source::SeekError;
use rodio::{ChannelCount, Decoder, Sample, SampleRate, Source};

/// Opus is decoded at 48 kHz, the rate its granule positions count in, so no position needs converting.
const RATE: u32 = GRANULE_RATE;
/// Audio decoded ahead of a seek target and thrown away, so the decoder has settled once the target plays; RFC 7845
/// asks for 80 ms.
const PREROLL: u64 = 3840;
/// How far from the end of the file the last page, whose granule gives the length, is looked for.
const TAIL_BYTES: u64 = 64 * 1024;
/// The capture pattern every Ogg page starts with.
const PAGE_MARK: &[u8] = b"OggS";

/// A song file as a source of samples: rodio's own decoders for every format they know, and Opus for Ogg Opus files.
pub fn open(path: &Path) -> Result<Box<dyn Source + Send>, String> {
  let file = File::open(path).map_err(|e| e.to_string())?;
  match Decoder::try_from(file) {
    Ok(decoder) => Ok(Box::new(decoder)),
    // only a file rodio cannot read is tried as Opus, and its own complaint is kept for files that are not Opus either
    Err(error) => OpusSource::open(path)
      .map(|source| Box::new(source) as Box<dyn Source + Send>)
      .map_err(|_| error.to_string()),
  }
}

/// An Ogg Opus stream decoded packet by packet, with the encoder delay cut from the front and the padding from the back.
pub struct OpusSource {
  reader: OggOpusReader<BufReader<File>>,
  head: OpusHead,
  decoder: OpusDecoder,
  trim: Trim,
  packet: OggPacket,
  /// The last decoded packet, channels interleaved.
  pcm: Vec<f32>,
  /// The part of `pcm` still to play.
  ready: Range<usize>,
  /// Frames still to throw away after a seek, the pre-roll and the rest of the packet before the target.
  discard: u64,
  channels: ChannelCount,
  duration: Option<Duration>,
}

impl OpusSource {
  pub fn open(path: &Path) -> opus_pure::Result<Self> {
    let mut file = File::open(path)?;
    let last_granule = last_granule(&mut file);
    file.seek(SeekFrom::Start(0))?;
    let reader = OggOpusReader::new(BufReader::new(file))?;
    let head = reader.head().clone();
    let decoder = head.decoder(rate())?;
    let width = usize::from(head.channel_count);
    let channels = ChannelCount::new(head.channel_count.into())
      .ok_or(opus_pure::Error::InvalidStream("a stream without channels"))?;
    let duration = last_granule.map(|granule| {
      let frames = granule.saturating_sub(head.pre_skip.into());
      Duration::from_secs_f64(frames as f64 / f64::from(RATE))
    });
    Ok(Self {
      trim: Trim::new(&head, rate(), width)?,
      reader,
      head,
      decoder,
      packet: OggPacket::default(),
      pcm: vec![0.0; MAX_PACKET_SAMPLES * width],
      ready: 0..0,
      discard: 0,
      channels,
      duration,
    })
  }

  fn width(&self) -> usize {
    usize::from(self.channels.get())
  }

  /// Reads and decodes the next packet; false at the end of the stream or at a broken page.
  fn next_block(&mut self) -> bool {
    match self.reader.read_packet_into(&mut self.packet) {
      Ok(true) => self.decode_packet(),
      Ok(false) => false,
      Err(error) => {
        eprintln!("cannot read the Opus stream: {error}");
        false
      }
    }
  }

  /// Decodes the packet read last into `pcm`, keeping what is audio and not to be thrown away.
  fn decode_packet(&mut self) -> bool {
    let samples = match self
      .decoder
      .decode(&self.packet.data, MAX_PACKET_SAMPLES, &mut self.pcm)
    {
      Ok(samples) => samples,
      Err(error) => {
        eprintln!("cannot decode an Opus packet: {error}");
        return false;
      }
    };
    let width = self.width();
    let audio = self.trim.keep_range(&self.packet, samples * width);
    let frames = ((audio.end - audio.start) / width) as u64;
    let dropped = self.discard.min(frames);
    self.discard -= dropped;
    self.ready = audio.start + dropped as usize * width..audio.end;
    true
  }
}

impl Iterator for OpusSource {
  type Item = Sample;

  fn next(&mut self) -> Option<Sample> {
    while self.ready.is_empty() {
      if !self.next_block() {
        return None;
      }
    }
    let sample = self.pcm[self.ready.start];
    self.ready.start += 1;
    Some(sample)
  }
}

impl Source for OpusSource {
  fn current_span_len(&self) -> Option<usize> {
    // the rate and channels never change within the stream
    None
  }

  fn channels(&self) -> ChannelCount {
    self.channels
  }

  fn sample_rate(&self) -> SampleRate {
    SampleRate::new(RATE).expect("48 kHz is not zero")
  }

  fn total_duration(&self) -> Option<Duration> {
    self.duration
  }

  /// Goes back to the start and counts whole packets up to the pre-roll without decoding them, which only needs their
  /// lengths, so a seek deep into an hour-long mix stays quick.
  fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
    let fail = |error: opus_pure::Error| SeekError::Other(Arc::new(error));
    let width = self.width();
    self.reader.rewind().map_err(fail)?;
    self.decoder.reset_state().map_err(fail)?;
    self.trim = Trim::new(&self.head, rate(), width).map_err(fail)?;
    self.ready = 0..0;
    self.discard = 0;
    let target = (position.as_secs_f64() * f64::from(RATE)) as u64;
    loop {
      if !self.reader.read_packet_into(&mut self.packet).map_err(fail)? {
        // past the end, so nothing is left to play
        return Ok(());
      }
      let samples = opus_pure::packet::samples_48k(&self.packet.data).map_err(fail)?;
      if self.trim.samples_emitted() + samples as u64 + PREROLL > target {
        break;
      }
      self.trim.keep_range(&self.packet, samples * width);
    }
    self.discard = target.saturating_sub(self.trim.samples_emitted());
    self.decode_packet();
    Ok(())
  }
}

/// The decode rate as the decoder takes it.
fn rate() -> i32 {
  RATE as i32
}

/// The granule position of the last page in the file, the stream's length at 48 kHz including the pre-skip.
fn last_granule(file: &mut File) -> Option<u64> {
  let length = file.seek(SeekFrom::End(0)).ok()?;
  file.seek(SeekFrom::Start(length.saturating_sub(TAIL_BYTES))).ok()?;
  let mut tail = Vec::new();
  file.read_to_end(&mut tail).ok()?;
  let page = tail.windows(PAGE_MARK.len()).rposition(|window| window == PAGE_MARK)?;
  // the granule is the eight little-endian bytes after the mark, the version and the header type
  let granule = tail.get(page + 6..page + 14)?;
  u64::try_from(i64::from_le_bytes(granule.try_into().ok()?)).ok()
}

#[cfg(test)]
mod tests {
  use std::time::Instant;

  use super::*;

  /// Plays the start of a real Ogg Opus file named by `AIO_OPUS_SAMPLE`, then seeks near its end and plays on.
  #[test]
  #[ignore = "needs an Opus file named by AIO_OPUS_SAMPLE"]
  fn plays_and_seeks_a_real_file() {
    let path = std::env::var("AIO_OPUS_SAMPLE").expect("AIO_OPUS_SAMPLE");
    let mut source = open(Path::new(&path)).expect("opens");
    let length = source.total_duration().expect("knows its length");
    let second = (source.sample_rate().get() * u32::from(source.channels().get())) as usize;
    let loud = source
      .by_ref()
      .take(second * 10)
      .filter(|sample| sample.abs() > 0.01)
      .count();
    assert!(loud > second, "the first ten seconds are not silent");
    let started = Instant::now();
    source
      .try_seek(length.saturating_sub(Duration::from_secs(30)))
      .expect("seeks");
    let seek_took = started.elapsed();
    let rest = source.count();
    let rest_secs = rest as f64 / second as f64;
    println!("length {length:?}, seek took {seek_took:?}, {rest_secs:.2} s left after seeking");
    assert!((rest_secs - 30.0).abs() < 0.1);
  }
}
