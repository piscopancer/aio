//! Streaming sites read through yt-dlp and plain HTTP, and the tags written into downloaded files with lofty.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::thread;

use lofty::config::WriteOptions;
use lofty::file::{AudioFile as _, TaggedFileExt as _};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::items::Timestamp;
use lofty::tag::{Accessor as _, Tag};
use serde_json::Value;

use super::covers::square_jpeg;
use super::tools::{ToolKit, command, fetch_error, last_error};
use crate::application::error::{AppError, AppResult};
use crate::application::ports::{FetchJob, FetchProgress, MediaSource, SpotifyTrack, VideoInfo};
use crate::application::views::CoverSource;
use crate::domain::download::{AudioQuality, Candidate, DownloadError, SongTags, VideoFacts, year_of};

/// How many search results are compared by length when matching a Spotify track.
const SEARCH_RESULTS: u32 = 5;
/// Marks the progress lines Aio asks yt-dlp for, among everything else it prints.
const PROGRESS_MARK: &str = "aio-progress";
/// Some sites send a stripped page to clients that do not look like a browser.
const BROWSER_AGENT: &str =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36";

pub struct YtDlpMedia {
  tools: Arc<ToolKit>,
  http: ureq::Agent,
}

impl YtDlpMedia {
  pub fn new(tools: Arc<ToolKit>) -> Self {
    let http = ureq::Agent::config_builder().user_agent(BROWSER_AGENT).build().into();
    Self { tools, http }
  }

  /// yt-dlp pointed at the fetched ffmpeg and deno, quiet apart from what is asked for.
  fn yt_dlp(&self) -> std::process::Command {
    let mut command = command(&self.tools.yt_dlp());
    command
      .arg("--ffmpeg-location")
      .arg(self.tools.dir())
      .arg("--js-runtimes")
      .arg(format!("deno:{}", self.tools.deno().display()))
      .args(["--no-warnings", "--no-playlist", "--encoding", "utf-8"]);
    command
  }

  fn json(&self, args: &[&str]) -> AppResult<Value> {
    let output = self.yt_dlp().args(args).output().map_err(fetch_error)?;
    if !output.status.success() {
      return Err(AppError::Fetch(last_error(&output.stderr)));
    }
    serde_json::from_slice(&output.stdout).map_err(fetch_error)
  }

  fn get_bytes(&self, url: &str) -> AppResult<Vec<u8>> {
    let mut bytes = Vec::new();
    self
      .http
      .get(url)
      .call()
      .map_err(fetch_error)?
      .into_body()
      .into_reader()
      .take(20 * 1024 * 1024)
      .read_to_end(&mut bytes)
      .map_err(fetch_error)?;
    Ok(bytes)
  }

  /// The image `cover` points to, cut to a centred square and saved as JPEG, the cover shape the app shows.
  fn square_cover(&self, cover: &CoverSource) -> AppResult<Vec<u8>> {
    let bytes = match cover {
      CoverSource::Web { url } => self.get_bytes(url)?,
      CoverSource::File { path } => std::fs::read(path).map_err(fetch_error)?,
    };
    let image = image::load_from_memory(&bytes).map_err(fetch_error)?;
    square_jpeg(&image).map_err(fetch_error)
  }
}

impl MediaSource for YtDlpMedia {
  fn video(&self, url: &str) -> AppResult<VideoInfo> {
    let info = self.json(&["--dump-single-json", "--skip-download", url])?;
    let text = |key: &str| info.get(key).and_then(Value::as_str).map(str::to_owned);
    let original_kbps = info
      .get("formats")
      .and_then(Value::as_array)
      .into_iter()
      .flatten()
      .filter(|f| f.get("vcodec").and_then(Value::as_str) == Some("none"))
      .filter(|f| f.get("ext").and_then(Value::as_str) == Some("m4a"))
      .filter_map(|f| f.get("abr").and_then(Value::as_f64))
      .fold(None, |best: Option<f64>, abr| Some(best.map_or(abr, |b| b.max(abr))))
      .map(|abr| abr.round() as u32);
    Ok(VideoInfo {
      url: text("webpage_url").unwrap_or_else(|| url.to_owned()),
      facts: VideoFacts {
        title: text("title").unwrap_or_default(),
        channel: text("channel").or_else(|| text("uploader")).unwrap_or_default(),
        artist: text("artist").or_else(|| {
          info
            .get("artists")
            .and_then(Value::as_array)
            .map(|all| all.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))
        }),
        track: text("track"),
        album: text("album"),
      },
      duration_ms: seconds_to_ms(info.get("duration")),
      thumbnail: text("thumbnail"),
      original_kbps,
      year: info
        .get("release_year")
        .and_then(Value::as_u64)
        .map(|year| year.to_string())
        .or_else(|| text("release_date"))
        .and_then(|date| year_of(&date)),
    })
  }

  fn search_songs(&self, query: &str) -> AppResult<Vec<Candidate>> {
    let encoded: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let music = format!("https://music.youtube.com/search?q={encoded}#songs");
    let items = format!("1:{SEARCH_RESULTS}");
    let plain = format!("ytsearch{SEARCH_RESULTS}:{query}");
    let result = self
      .json(&[
        "--flat-playlist",
        "--yes-playlist",
        "--playlist-items",
        &items,
        "--dump-single-json",
        &music,
      ])
      .or_else(|_| self.json(&["--flat-playlist", "--yes-playlist", "--dump-single-json", &plain]))?;
    Ok(
      result
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
          let id = entry.get("id").and_then(Value::as_str)?;
          Some(Candidate {
            url: format!("https://music.youtube.com/watch?v={id}"),
            duration_ms: entry
              .get("duration")
              .filter(|d| d.is_number())
              .map(|d| seconds_to_ms(Some(d))),
          })
        })
        .collect(),
    )
  }

  /// Reads the track from Spotify's embed page, which carries its data as JSON and needs no account.
  fn spotify_track(&self, id: &str) -> AppResult<SpotifyTrack> {
    let page = self
      .http
      .get(&format!("https://open.spotify.com/embed/track/{id}"))
      .call()
      .map_err(fetch_error)?
      .into_body()
      .read_to_string()
      .map_err(fetch_error)?;
    let data = page
      .split_once(r#"<script id="__NEXT_DATA__" type="application/json">"#)
      .and_then(|(_, rest)| rest.split_once("</script>"))
      .map(|(json, _)| json)
      .ok_or_else(|| AppError::Fetch("Spotify did not describe this track".into()))?;
    let data: Value = serde_json::from_str(data).map_err(fetch_error)?;
    let entity = data
      .pointer("/props/pageProps/state/data/entity")
      .ok_or_else(|| AppError::Fetch("Spotify did not describe this track".into()))?;
    let title = entity
      .get("name")
      .or_else(|| entity.get("title"))
      .and_then(Value::as_str)
      .ok_or(DownloadError::NoMatch)?
      .to_owned();
    let artists = entity
      .get("artists")
      .and_then(Value::as_array)
      .into_iter()
      .flatten()
      .filter_map(|a| a.get("name").and_then(Value::as_str).map(str::to_owned))
      .collect();
    let cover = ["/visualIdentity/image", "/coverArt/sources"]
      .iter()
      .filter_map(|pointer| entity.pointer(pointer).and_then(Value::as_array))
      .flatten()
      .max_by_key(|image| {
        image
          .get("maxWidth")
          .or_else(|| image.get("width"))
          .and_then(Value::as_u64)
          .unwrap_or(0)
      })
      .and_then(|image| image.get("url").and_then(Value::as_str).map(str::to_owned));
    Ok(SpotifyTrack {
      title,
      artists,
      album: None,
      duration_ms: entity
        .get("duration")
        .and_then(Value::as_u64)
        .map_or(0, |ms| u32::try_from(ms).unwrap_or(u32::MAX)),
      cover,
      year: entity
        .pointer("/releaseDate/isoString")
        .and_then(Value::as_str)
        .and_then(year_of),
    })
  }

  fn fetch(&self, job: &FetchJob<'_>, progress: &mut dyn FnMut(FetchProgress)) -> AppResult<()> {
    let stem = job.target.with_extension("");
    // a percent sign in the song name would otherwise start an output template field
    let output = format!("{}.%(ext)s", stem.display().to_string().replace('%', "%%"));
    let template = format!(
      "download:{PROGRESS_MARK} %(progress.downloaded_bytes)s %(progress.total_bytes,progress.total_bytes_estimate)s"
    );
    let mut command = self.yt_dlp();
    command.args([
      "--newline",
      "--no-mtime",
      "--progress",
      "--progress-template",
      &template,
      "-o",
      &output,
    ]);
    match job.quality {
      AudioQuality::Original => command.args(["-f", "bestaudio[ext=m4a]/bestaudio[acodec^=mp4a]"]),
      quality => command.args([
        "-f",
        "bestaudio",
        "--extract-audio",
        "--audio-format",
        "mp3",
        "--audio-quality",
        &format!("{}K", quality.kbps(0)),
      ]),
    };
    let mut child = command
      .arg(job.video_url)
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .map_err(fetch_error)?;
    let mut stderr = child
      .stderr
      .take()
      .ok_or_else(|| fetch_error("yt-dlp has no error output"))?;
    let errors = thread::spawn(move || {
      let mut text = Vec::new();
      let _ = stderr.read_to_end(&mut text);
      text
    });
    let stdout = child.stdout.take().ok_or_else(|| fetch_error("yt-dlp has no output"))?;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
      if let Some(numbers) = line.trim().strip_prefix(PROGRESS_MARK) {
        let mut parts = numbers.split_whitespace().map(|n| n.parse::<f64>().ok());
        if let (Some(Some(done)), Some(Some(total))) = (parts.next(), parts.next())
          && total > 0.0
        {
          progress(FetchProgress::Downloading((done / total).clamp(0.0, 1.0) as f32));
        }
      } else if line.starts_with("[ExtractAudio]") || line.starts_with("[Fixup") {
        progress(FetchProgress::Converting);
      }
    }
    let status = child.wait().map_err(fetch_error)?;
    let errors = errors.join().unwrap_or_default();
    if !status.success() {
      return Err(AppError::Fetch(last_error(&errors)));
    }
    if !job.target.exists() {
      return Err(AppError::Fetch(format!(
        "yt-dlp did not write {}",
        job.target.display()
      )));
    }
    Ok(())
  }

  fn write_tags(&self, path: &Path, tags: &SongTags, cover: Option<&CoverSource>) -> AppResult<()> {
    let mut file = lofty::read_from_path(path).map_err(fetch_error)?;
    if file.primary_tag().is_none() {
      let kind = file.primary_tag_type();
      file.insert_tag(Tag::new(kind));
    }
    let tag = file
      .primary_tag_mut()
      .ok_or_else(|| AppError::Fetch(format!("{} cannot hold tags", path.display())))?;
    tag.set_title(tags.title().to_owned());
    tag.set_artist(tags.artist().to_owned());
    match tags.album() {
      Some(album) => tag.set_album(album.to_owned()),
      None => tag.remove_album(),
    }
    match tags.year() {
      Some(year) => tag.set_date(Timestamp {
        year,
        ..Timestamp::default()
      }),
      None => tag.remove_date(),
    }
    // a missing cover only costs the artwork, not the song
    if let Some(cover) = cover.and_then(|cover| {
      self
        .square_cover(cover)
        .inspect_err(|error| eprintln!("cover of {} skipped: {error}", path.display()))
        .ok()
    }) {
      tag.remove_picture_type(PictureType::CoverFront);
      tag.push_picture(
        Picture::unchecked(cover)
          .pic_type(PictureType::CoverFront)
          .mime_type(MimeType::Jpeg)
          .build(),
      );
    }
    file.save_to_path(path, WriteOptions::default()).map_err(fetch_error)
  }
}

fn seconds_to_ms(value: Option<&Value>) -> u32 {
  value.and_then(Value::as_f64).map_or(0, |seconds| {
    (seconds * 1000.0).round().clamp(0.0, f64::from(u32::MAX)) as u32
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::application::ports::DownloadTools as _;
  use crate::domain::download::{SongLink, guess_tags};

  /// Runs the whole pipeline against the real sites, keeping the tools in `<AIO_DATA_DIR>/tools`: `cargo test -- --ignored live_download`.
  #[test]
  #[ignore = "downloads the tools and a song from the internet"]
  fn live_download() {
    let dir = std::path::PathBuf::from(std::env::var("AIO_DATA_DIR").expect("set AIO_DATA_DIR"));
    let tools = Arc::new(ToolKit::new(dir.join("tools")));
    tools.install_missing(&mut |_| {}).unwrap();
    assert!(tools.versions().complete(), "{:?}", tools.versions());
    let media = YtDlpMedia::new(tools);

    let link = SongLink::parse("https://www.youtube.com/watch?v=dQw4w9WgXcQ").unwrap();
    let video = media.video(&link.url()).unwrap();
    println!(
      "{:?} {:?} {:?}",
      guess_tags(&video.facts),
      video.original_kbps,
      video.thumbnail
    );

    let track = media.spotify_track("4cOdK2wGLETKBW3PvgPWqT").unwrap();
    let hits = media
      .search_songs(&format!("{} {}", track.artists.join(" "), track.title))
      .unwrap();
    println!("{} by {:?}, {} hits", track.title, track.artists, hits.len());
    assert!(!hits.is_empty());

    for quality in [AudioQuality::Original, AudioQuality::Mp3Low] {
      let target = std::env::temp_dir().join(format!("aio live 100% test.{}", quality.extension()));
      let _ = std::fs::remove_file(&target);
      let mut steps = 0;
      media
        .fetch(
          &FetchJob {
            video_url: &video.url,
            quality,
            target: &target,
          },
          &mut |_| steps += 1,
        )
        .unwrap();
      let tags = SongTags::new("Never Gonna Give You Up", "Rick Astley", Some("Whenever"), Some(1987)).unwrap();
      media
        .write_tags(
          &target,
          &tags,
          video.thumbnail.clone().map(|url| CoverSource::Web { url }).as_ref(),
        )
        .unwrap();
      let read = lofty::read_from_path(&target).unwrap();
      let tag = read.primary_tag().unwrap();
      assert_eq!(tag.artist().as_deref(), Some("Rick Astley"));
      assert_eq!(tag.pictures().len(), 1);
      println!(
        "{quality:?}: {} bytes, {steps} progress steps",
        std::fs::metadata(&target).unwrap().len()
      );
    }
  }
}
