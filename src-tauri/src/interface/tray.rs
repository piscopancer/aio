//! The app's icon in the notification area. A click brings the window back; its menu names the playing song, controls
//! the player, shows the window and quits, which is the one way out while closing the window only hides it. The menu
//! follows the player's state, and its icons the light or dark theme of the system.

use std::sync::{Arc, Mutex, MutexGuard};

use tauri::image::Image;
use tauri::menu::{IconMenuItem, Menu, MenuEvent, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Theme, Wry};

use super::icons::{coverage, pixels};
use super::window::{self, MAIN};
use crate::application::playback::{PlayerCommand, PlayerHandle};
use crate::application::views::PlayerState;

/// Size of a menu icon at 100% scale.
const ICON_PX: f64 = 16.0;
/// The longest song line in characters; longer ones end in an ellipsis, so the menu stays a sensible width.
const SONG_CHARS: usize = 64;
/// What the song line says while nothing is loaded.
const NOTHING: &str = "Nothing playing";
/// Colour of the icons on the menu of each theme: white on dark, near black on light.
const ON_DARK: [u8; 3] = [255, 255, 255];
const ON_LIGHT: [u8; 3] = [26, 26, 26];

/// The menu's items that do something, by the id the menu reports a click with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
  PlayPause,
  Next,
  Previous,
  Show,
  Quit,
}

const ACTIONS: [Action; 5] = [
  Action::PlayPause,
  Action::Next,
  Action::Previous,
  Action::Show,
  Action::Quit,
];

impl Action {
  fn id(self) -> &'static str {
    match self {
      Action::PlayPause => "play-pause",
      Action::Next => "next",
      Action::Previous => "previous",
      Action::Show => "show",
      Action::Quit => "quit",
    }
  }

  fn from_id(id: &str) -> Option<Self> {
    ACTIONS.into_iter().find(|action| action.id() == id)
  }
}

/// What the menu shows, taken from the player's state.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Shown {
  /// "Artist — Title" of the loaded song, none while nothing is.
  song: Option<String>,
  playing: bool,
}

impl Shown {
  fn of(state: &PlayerState) -> Self {
    Self {
      song: state.current_track().map(|track| {
        let credit = track.credit();
        if credit.is_empty() {
          track.title.clone()
        } else {
          format!("{credit} — {}", track.title)
        }
      }),
      playing: state.is_playing,
    }
  }

  /// The song line as the menu shows it: shortened, and with ampersands doubled, since a single one marks the
  /// following letter as the item's access key.
  fn song_line(&self) -> String {
    let song = self.song.as_deref().unwrap_or(NOTHING);
    let short: String = if song.chars().count() > SONG_CHARS {
      song.chars().take(SONG_CHARS - 1).chain(['…']).collect()
    } else {
      song.to_owned()
    };
    short.replace('&', "&&")
  }
}

/// The icons of every item, drawn for one theme.
struct Icons {
  song: Image<'static>,
  play: Image<'static>,
  pause: Image<'static>,
  next: Image<'static>,
  previous: Image<'static>,
  show: Image<'static>,
  quit: Image<'static>,
}

impl Icons {
  fn draw(size: u32, theme: Theme) -> Option<Self> {
    let [red, green, blue] = if theme == Theme::Light { ON_LIGHT } else { ON_DARK };
    let icon = |svg: &[u8]| {
      let rgba = coverage(svg, size)?
        .into_iter()
        .flat_map(|alpha| [red, green, blue, alpha])
        .collect();
      Some(Image::new_owned(rgba, size, size))
    };
    Some(Self {
      song: icon(include_bytes!("../../icons/tray/song.svg"))?,
      play: icon(include_bytes!("../../icons/taskbar/play.svg"))?,
      pause: icon(include_bytes!("../../icons/taskbar/pause.svg"))?,
      next: icon(include_bytes!("../../icons/taskbar/next.svg"))?,
      previous: icon(include_bytes!("../../icons/taskbar/previous.svg"))?,
      show: icon(include_bytes!("../../icons/tray/show.svg"))?,
      quit: icon(include_bytes!("../../icons/tray/quit.svg"))?,
    })
  }

  /// The icon of the play and pause item, which shows what a click on it does.
  fn toggle(&self, playing: bool) -> Image<'static> {
    if playing { self.pause.clone() } else { self.play.clone() }
  }
}

/// The live tray icon and the items of its menu that change.
struct Tray {
  icon: TrayIcon,
  song: IconMenuItem<Wry>,
  play_pause: IconMenuItem<Wry>,
  next: IconMenuItem<Wry>,
  previous: IconMenuItem<Wry>,
  show: IconMenuItem<Wry>,
  quit: IconMenuItem<Wry>,
  size: u32,
  icons: Mutex<Icons>,
  shown: Mutex<Shown>,
}

impl Tray {
  fn icons(&self) -> MutexGuard<'_, Icons> {
    // a poisoned lock only means an update panicked; the next one replaces what it held
    self.icons.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  fn shown(&self) -> MutexGuard<'_, Shown> {
    self.shown.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  /// Puts `shown` on the song line, the tooltip and the player items.
  fn show(&self, shown: &Shown) -> tauri::Result<()> {
    let line = shown.song_line();
    self.song.set_text(&line)?;
    self.icon.set_tooltip(Some(shown.song.as_deref().unwrap_or("Aio")))?;
    let has_track = shown.song.is_some();
    for item in [&self.play_pause, &self.next, &self.previous] {
      item.set_enabled(has_track)?;
    }
    self.play_pause.set_text(if shown.playing { "Pause" } else { "Play" })?;
    self.play_pause.set_icon(Some(self.icons().toggle(shown.playing)))
  }

  /// Puts the current icons on every item.
  fn paint(&self) -> tauri::Result<()> {
    let icons = self.icons();
    self.song.set_icon(Some(icons.song.clone()))?;
    self.play_pause.set_icon(Some(icons.toggle(self.shown().playing)))?;
    self.next.set_icon(Some(icons.next.clone()))?;
    self.previous.set_icon(Some(icons.previous.clone()))?;
    self.show.set_icon(Some(icons.show.clone()))?;
    self.quit.set_icon(Some(icons.quit.clone()))
  }
}

/// Puts the icon into the notification area; called once during setup, after the window exists.
pub fn attach(app: &AppHandle, player: Arc<PlayerHandle>) -> tauri::Result<()> {
  let window = app.get_webview_window(MAIN);
  let scale = window.as_ref().and_then(|w| w.scale_factor().ok()).unwrap_or(1.0);
  let theme = window.as_ref().and_then(|w| w.theme().ok()).unwrap_or(Theme::Dark);
  let size = pixels(ICON_PX, scale);
  let icons = Icons::draw(size, theme).ok_or_else(|| tauri::Error::InvalidIcon(std::io::Error::other("tray icons")))?;
  let shown = Shown::of(&player.state());

  let item = |action: Option<Action>, text: &str, icon: &Image<'static>, enabled: bool| {
    // the song line does nothing, so its id is one no click is told apart by
    let id = action.map_or("song", Action::id);
    IconMenuItem::with_id(app, id, text, enabled, Some(icon.clone()), None::<&str>)
  };
  let has_track = shown.song.is_some();
  let song = item(None, &shown.song_line(), &icons.song, false)?;
  let play_pause = item(Some(Action::PlayPause), "Play", &icons.play, has_track)?;
  let next = item(Some(Action::Next), "Next", &icons.next, has_track)?;
  let previous = item(Some(Action::Previous), "Previous", &icons.previous, has_track)?;
  let show = item(Some(Action::Show), "Show Aio", &icons.show, true)?;
  let quit = item(Some(Action::Quit), "Quit", &icons.quit, true)?;
  let menu = Menu::with_items(
    app,
    &[
      &song,
      &PredefinedMenuItem::separator(app)?,
      &play_pause,
      &next,
      &previous,
      &PredefinedMenuItem::separator(app)?,
      &show,
      &quit,
    ],
  )?;

  let mut builder = TrayIconBuilder::with_id(MAIN)
    .tooltip("Aio")
    .menu(&menu)
    .show_menu_on_left_click(false)
    .on_menu_event(move |app, event| on_menu(app, &event, &player))
    .on_tray_icon_event(|tray, event| {
      if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
      } = event
      {
        window::reveal(tray.app_handle());
      }
    });
  if let Some(icon) = app.default_window_icon() {
    builder = builder.icon(icon.clone());
  }
  let tray = Tray {
    icon: builder.build(app)?,
    song,
    play_pause,
    next,
    previous,
    show,
    quit,
    size,
    icons: Mutex::new(icons),
    shown: Mutex::new(shown.clone()),
  };
  tray.show(&shown)?;
  app.manage(tray);
  Ok(())
}

fn on_menu(app: &AppHandle, event: &MenuEvent, player: &PlayerHandle) {
  let Some(action) = Action::from_id(event.id().as_ref()) else {
    return;
  };
  match action {
    Action::PlayPause => player.send(PlayerCommand::Toggle),
    Action::Next => player.send(PlayerCommand::Next),
    Action::Previous => player.send(PlayerCommand::Previous),
    Action::Show => window::reveal(app),
    Action::Quit => app.exit(0),
  }
}

/// Shows the player's new state in the menu, from whichever thread reports it.
pub fn show(app: &AppHandle, state: &PlayerState) {
  let Some(tray) = app.try_state::<Tray>() else { return };
  let next = Shown::of(state);
  let mut shown = tray.shown();
  if *shown == next {
    return;
  }
  *shown = next.clone();
  drop(shown);
  if let Err(error) = tray.show(&next) {
    eprintln!("the tray menu missed a player change: {error}");
  }
}

/// Redraws the menu's icons for the system's new light or dark theme.
pub fn retheme(app: &AppHandle, theme: Theme) {
  let Some(tray) = app.try_state::<Tray>() else { return };
  let Some(icons) = Icons::draw(tray.size, theme) else {
    return;
  };
  *tray.icons() = icons;
  if let Err(error) = tray.paint() {
    eprintln!("the tray menu kept its old icons: {error}");
  }
}
