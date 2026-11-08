//! The download tools (yt-dlp, ffmpeg, deno), fetched on first use into the app data folder and run as child
//! processes without a console window.

use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Mutex;

use crate::application::error::{AppError, AppResult};
use crate::application::ports::DownloadTools;
use crate::application::views::{Tool, ToolInstall, ToolVersions};
use crate::domain::download::DownloadError;

/// Where a tool comes from and which files of its archive are kept.
struct Release {
  tool: Tool,
  url: &'static str,
  /// Names of the executables to keep; a direct download is saved under the first one.
  files: &'static [&'static str],
  zipped: bool,
}

#[cfg(windows)]
const RELEASES: &[Release] = &[
  Release {
    tool: Tool::YtDlp,
    url: "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
    files: &["yt-dlp.exe"],
    zipped: false,
  },
  Release {
    tool: Tool::Ffmpeg,
    url: "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
    files: &["ffmpeg.exe", "ffprobe.exe"],
    zipped: true,
  },
  Release {
    tool: Tool::Deno,
    url: "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip",
    files: &["deno.exe"],
    zipped: true,
  },
];

#[cfg(not(windows))]
const RELEASES: &[Release] = &[];

/// Bytes between two progress reports while a tool downloads.
const REPORT_EVERY: u32 = 512 * 1024;

/// The tools folder and the versions found there, cached until a tool changes.
pub struct ToolKit {
  dir: PathBuf,
  versions: Mutex<Option<ToolVersions>>,
}

impl ToolKit {
  pub fn new(dir: PathBuf) -> Self {
    Self {
      dir,
      versions: Mutex::new(None),
    }
  }

  pub fn yt_dlp(&self) -> PathBuf {
    self.dir.join(exe("yt-dlp"))
  }

  pub fn deno(&self) -> PathBuf {
    self.dir.join(exe("deno"))
  }

  /// ffmpeg and ffprobe live side by side in the tools folder, which is what yt-dlp is pointed at.
  pub fn dir(&self) -> &Path {
    &self.dir
  }

  fn path(&self, tool: Tool) -> PathBuf {
    match tool {
      Tool::YtDlp => self.yt_dlp(),
      Tool::Ffmpeg => self.dir.join(exe("ffmpeg")),
      Tool::Deno => self.deno(),
    }
  }

  fn read_versions(&self) -> ToolVersions {
    let first_line = |tool: Tool, arg: &str| {
      let path = self.path(tool);
      if !path.exists() {
        return None;
      }
      let output = command(&path).arg(arg).output().ok()?;
      let text = String::from_utf8_lossy(&output.stdout);
      text.lines().next().map(|line| line.trim().to_owned())
    };
    ToolVersions {
      yt_dlp: first_line(Tool::YtDlp, "--version"),
      // "ffmpeg version N-12345-g1234 Copyright ..." becomes "N-12345-g1234"
      ffmpeg: first_line(Tool::Ffmpeg, "-version")
        .map(|line| line.split_whitespace().nth(2).unwrap_or(&line).to_owned()),
      // "deno 2.5.0 (stable, release, x86_64-pc-windows-msvc)" becomes "2.5.0"
      deno: first_line(Tool::Deno, "--version").map(|line| line.split_whitespace().nth(1).unwrap_or(&line).to_owned()),
    }
  }

  fn forget_versions(&self) {
    *self.versions.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
  }

  fn install(&self, release: &Release, progress: &mut dyn FnMut(ToolInstall)) -> AppResult<()> {
    fs::create_dir_all(&self.dir).map_err(fetch_error)?;
    let response = ureq::get(release.url).call().map_err(fetch_error)?;
    let total_bytes = response
      .headers()
      .get("content-length")
      .and_then(|v| v.to_str().ok())
      .and_then(|v| v.parse::<u32>().ok());
    let download = self.dir.join(format!("{}.download", release.files[0]));
    {
      let mut reader = response.into_body().into_reader();
      let mut writer = BufWriter::new(File::create(&download).map_err(fetch_error)?);
      let mut buffer = vec![0u8; 64 * 1024];
      let (mut done_bytes, mut reported) = (0u32, 0u32);
      progress(ToolInstall {
        tool: release.tool,
        done_bytes,
        total_bytes,
      });
      loop {
        let read = reader.read(&mut buffer).map_err(fetch_error)?;
        if read == 0 {
          break;
        }
        writer.write_all(&buffer[..read]).map_err(fetch_error)?;
        done_bytes = done_bytes.saturating_add(u32::try_from(read).unwrap_or(u32::MAX));
        if done_bytes - reported >= REPORT_EVERY {
          reported = done_bytes;
          progress(ToolInstall {
            tool: release.tool,
            done_bytes,
            total_bytes,
          });
        }
      }
      writer.flush().map_err(fetch_error)?;
    }
    let result = if release.zipped {
      self.unzip(&download, release.files)
    } else {
      fs::rename(&download, self.dir.join(release.files[0])).map_err(fetch_error)
    };
    let _ = fs::remove_file(&download);
    result
  }

  /// Takes the wanted executables out of an archive, wherever they sit inside it.
  fn unzip(&self, archive: &Path, wanted: &[&str]) -> AppResult<()> {
    let mut zip = zip::ZipArchive::new(File::open(archive).map_err(fetch_error)?).map_err(fetch_error)?;
    let mut found = 0;
    for index in 0..zip.len() {
      let mut entry = zip.by_index(index).map_err(fetch_error)?;
      let Some(name) = entry
        .enclosed_name()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
      else {
        continue;
      };
      if entry.is_file() && wanted.contains(&name.as_str()) {
        let mut out = File::create(self.dir.join(&name)).map_err(fetch_error)?;
        io::copy(&mut entry, &mut out).map_err(fetch_error)?;
        found += 1;
      }
    }
    if found < wanted.len() {
      return Err(AppError::Fetch(format!(
        "{} lacks {}",
        archive.display(),
        wanted.join(", ")
      )));
    }
    Ok(())
  }
}

impl DownloadTools for ToolKit {
  fn versions(&self) -> ToolVersions {
    let mut cached = self.versions.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    cached.get_or_insert_with(|| self.read_versions()).clone()
  }

  fn install_missing(&self, progress: &mut dyn FnMut(ToolInstall)) -> AppResult<()> {
    if RELEASES.is_empty() {
      return Err(DownloadError::UnsupportedPlatform.into());
    }
    for release in RELEASES {
      if !self.path(release.tool).exists() {
        self.install(release, progress)?;
      }
    }
    self.forget_versions();
    Ok(())
  }

  fn update(&self) -> AppResult<()> {
    let output = command(&self.yt_dlp()).arg("-U").output().map_err(fetch_error)?;
    self.forget_versions();
    if output.status.success() {
      Ok(())
    } else {
      Err(AppError::Fetch(last_error(&output.stderr)))
    }
  }
}

/// A child process that opens no console window on Windows.
pub fn command(program: &Path) -> Command {
  #[allow(unused_mut)]
  let mut command = Command::new(program);
  #[cfg(windows)]
  {
    use std::os::windows::process::CommandExt as _;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
  }
  command
}

/// Stops a tool together with the processes it started, such as the ffmpeg yt-dlp runs.
pub fn kill_tree(child: &mut Child) {
  #[cfg(windows)]
  {
    let pid = child.id().to_string();
    let _ = command(Path::new("taskkill")).args(["/T", "/F", "/PID", &pid]).status();
  }
  let _ = child.kill();
}

/// The most telling line a failed tool printed: its last "ERROR:" line, else its last line.
pub fn last_error(stderr: &[u8]) -> String {
  let text = String::from_utf8_lossy(stderr);
  let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
  lines
    .iter()
    .rev()
    .find_map(|line| line.strip_prefix("ERROR:"))
    .or(lines.last().copied())
    .unwrap_or("the tool failed without saying why")
    .trim()
    .to_owned()
}

pub fn fetch_error(error: impl std::fmt::Display) -> AppError {
  AppError::Fetch(error.to_string())
}

fn exe(name: &str) -> String {
  if cfg!(windows) {
    format!("{name}.exe")
  } else {
    name.to_owned()
  }
}
