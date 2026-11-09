//! The window's media session on Windows: the system media flyout, the lock screen and the keyboard media keys see the
//! playing song with its cover and position, and their play, pause, previous, next and seeking reach the player.

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use tauri::WebviewWindow;
use windows::Foundation::TypedEventHandler;
use windows::Media::{
  MediaPlaybackStatus, MediaPlaybackType, PlaybackPositionChangeRequestedEventArgs, SystemMediaTransportControls,
  SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
  SystemMediaTransportControlsTimelineProperties,
};
use windows::Storage::StorageFile;
use windows::Storage::Streams::RandomAccessStreamReference;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
use windows_core::{HSTRING, Ref};

use crate::application::playback::{PlayerCommand, PlayerHandle};
use crate::application::views::{PlayerState, Track};

/// How often the position is sent while playing; the system moves it on by itself in between, as its guidance suggests.
const TIMELINE_SYNC: Duration = Duration::from_secs(5);

/// What the flyout shows of the loaded song, so the details are sent again only when the song or its tags change.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Details {
  title: String,
  artist: String,
  album: String,
  cover: Option<String>,
  duration_ms: u32,
}

impl Details {
  fn of(track: &Track) -> Self {
    Self {
      title: track.title.clone(),
      // the same credit line the player bar shows
      artist: track
        .artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", "),
      album: track.album.clone().unwrap_or_default(),
      cover: track.cover.clone(),
      duration_ms: track.duration_ms,
    }
  }
}

/// The details and position last sent to the system.
#[derive(Default)]
struct Shown {
  details: Option<Details>,
  synced_at: Option<Instant>,
}

struct Session {
  controls: SystemMediaTransportControls,
  shown: Mutex<Shown>,
}

/// The session lives as long as the app; its controls are agile, so any thread that reports the player may update them.
static SESSION: OnceLock<Session> = OnceLock::new();

/// Opens the media session of `window` and routes its buttons and seeking to the player; called during setup.
pub fn attach(window: &WebviewWindow, player: Arc<PlayerHandle>) {
  let Ok(raw) = window.hwnd() else { return };
  if let Err(error) = open(HWND(raw.0), player) {
    eprintln!("no media session: {error}");
  }
}

fn open(hwnd: HWND, player: Arc<PlayerHandle>) -> windows_core::Result<()> {
  let interop = windows_core::factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()?;
  // SAFETY: the handle belongs to the live main window, which outlives the session
  let controls: SystemMediaTransportControls = unsafe { interop.GetForWindow(hwnd) }?;
  controls.SetIsEnabled(true)?;
  controls.SetPlaybackStatus(MediaPlaybackStatus::Closed)?;
  let buttons = player.clone();
  controls.ButtonPressed(&TypedEventHandler::new(
    move |_, args: Ref<SystemMediaTransportControlsButtonPressedEventArgs>| {
      if let Some(command) = command(args.ok()?.Button()?, &buttons.state()) {
        buttons.send(command);
      }
      Ok(())
    },
  ))?;
  controls.PlaybackPositionChangeRequested(&TypedEventHandler::new(
    move |_, args: Ref<PlaybackPositionChangeRequestedEventArgs>| {
      let position = Duration::from(args.ok()?.RequestedPlaybackPosition()?);
      player.send(PlayerCommand::Seek(
        u32::try_from(position.as_millis()).unwrap_or(u32::MAX),
      ));
      Ok(())
    },
  ))?;
  let _ = SESSION.set(Session {
    controls,
    shown: Mutex::default(),
  });
  Ok(())
}

/// The player command for a pressed media button; play and pause are separate keys, so each one acts only when it
/// changes something.
fn command(button: SystemMediaTransportControlsButton, state: &PlayerState) -> Option<PlayerCommand> {
  match button {
    SystemMediaTransportControlsButton::Play if !state.is_playing => Some(PlayerCommand::Toggle),
    SystemMediaTransportControlsButton::Pause if state.is_playing => Some(PlayerCommand::Toggle),
    SystemMediaTransportControlsButton::Next => Some(PlayerCommand::Next),
    SystemMediaTransportControlsButton::Previous => Some(PlayerCommand::Previous),
    _ => None,
  }
}

/// Shows the player's new state in the session, from whichever thread reports it.
pub fn show(state: &PlayerState) {
  let Some(session) = SESSION.get() else { return };
  if let Err(error) = session.show(state) {
    eprintln!("media session not updated: {error}");
  }
}

/// Moves the session's position along while playing, no more often than `TIMELINE_SYNC`.
pub fn progress(position_ms: u32) {
  let Some(session) = SESSION.get() else { return };
  if let Err(error) = session.progress(position_ms) {
    eprintln!("media session position not updated: {error}");
  }
}

impl Session {
  fn shown(&self) -> MutexGuard<'_, Shown> {
    // a poisoned lock only means an update panicked; the next one sends everything again
    self.shown.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  fn show(&self, state: &PlayerState) -> windows_core::Result<()> {
    let track = state
      .index
      .and_then(|index| state.queue.get(usize::try_from(index).ok()?))
      .map(|item| &item.track);
    let mut shown = self.shown();
    let Some(track) = track else {
      // with nothing loaded the session leaves the flyout
      if shown.details.take().is_some() {
        self.controls.DisplayUpdater()?.ClearAll()?;
      }
      shown.synced_at = None;
      return self.controls.SetPlaybackStatus(MediaPlaybackStatus::Closed);
    };
    let controls = &self.controls;
    controls.SetIsPlayEnabled(true)?;
    controls.SetIsPauseEnabled(true)?;
    controls.SetIsPreviousEnabled(true)?;
    controls.SetIsNextEnabled(true)?;
    controls.SetPlaybackStatus(if state.is_playing {
      MediaPlaybackStatus::Playing
    } else {
      MediaPlaybackStatus::Paused
    })?;
    let details = Details::of(track);
    if shown.details.as_ref() != Some(&details) {
      self.describe(&details)?;
      shown.details = Some(details);
    }
    self.sync(&mut shown, state.position_ms)
  }

  fn progress(&self, position_ms: u32) -> windows_core::Result<()> {
    let mut shown = self.shown();
    let due = shown.synced_at.is_none_or(|at| at.elapsed() >= TIMELINE_SYNC);
    if due {
      self.sync(&mut shown, position_ms)
    } else {
      Ok(())
    }
  }

  fn describe(&self, details: &Details) -> windows_core::Result<()> {
    let updater = self.controls.DisplayUpdater()?;
    updater.ClearAll()?;
    updater.SetType(MediaPlaybackType::Music)?;
    let music = updater.MusicProperties()?;
    music.SetTitle(&HSTRING::from(&details.title))?;
    music.SetArtist(&HSTRING::from(&details.artist))?;
    music.SetAlbumTitle(&HSTRING::from(&details.album))?;
    // a cover that cannot be read leaves the flyout without a picture, not without the song
    if let Some(cover) = details.cover.as_deref().and_then(|path| thumbnail(path).ok()) {
      updater.SetThumbnail(&cover)?;
    }
    updater.Update()
  }

  /// Sends the song's length and the position in it, which also tells the system where seeking may go.
  fn sync(&self, shown: &mut Shown, position_ms: u32) -> windows_core::Result<()> {
    let Some(details) = &shown.details else { return Ok(()) };
    let end = Duration::from_millis(details.duration_ms.into());
    let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
    timeline.SetStartTime(Duration::ZERO.into())?;
    timeline.SetMinSeekTime(Duration::ZERO.into())?;
    timeline.SetEndTime(end.into())?;
    timeline.SetMaxSeekTime(end.into())?;
    timeline.SetPosition(Duration::from_millis(position_ms.into()).min(end).into())?;
    self.controls.UpdateTimelineProperties(&timeline)?;
    shown.synced_at = Some(Instant::now());
    Ok(())
  }
}

/// The cover file as a picture the system reads by itself.
fn thumbnail(path: &str) -> windows_core::Result<RandomAccessStreamReference> {
  let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path))?.join()?;
  RandomAccessStreamReference::CreateFromFile(&file)
}
