//! Player buttons in the window's taskbar preview on Windows: the sound, previous, play or pause, next and the repeat mode, the
//! way music players offer them; they follow the player's state and send it commands.

use std::cell::RefCell;
use std::sync::Arc;

use resvg::{tiny_skia, usvg};
use tauri::{AppHandle, WebviewWindow};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
  BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, HBITMAP,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Shell::{
  DefSubclassProc, ITaskbarList3, SetWindowSubclass, THB_FLAGS, THB_ICON, THB_TOOLTIP, THBF_DISABLED, THBF_ENABLED,
  THBN_CLICKED, THUMBBUTTON, TaskbarList,
};
use windows::Win32::UI::WindowsAndMessaging::{
  CreateIconIndirect, HICON, ICONINFO, RegisterWindowMessageW, WM_COMMAND,
};
use windows::core::w;

use crate::application::playback::{PlayerCommand, PlayerHandle};
use crate::application::views::PlayerState;
use crate::domain::playback::RepeatMode;

/// Size of a taskbar button's icon at 100% scale.
const ICON_PX: f64 = 16.0;
/// Id of this window subclass among others on the same window.
const SUBCLASS_ID: usize = 1;

/// The buttons, in the order the preview shows them; their index is their id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Button {
  Mute,
  Previous,
  PlayPause,
  Next,
  Repeat,
}

const BUTTONS: [Button; 5] = [
  Button::Mute,
  Button::Previous,
  Button::PlayPause,
  Button::Next,
  Button::Repeat,
];

impl Button {
  fn id(self) -> u32 {
    self as u32
  }

  fn from_id(id: u32) -> Option<Self> {
    BUTTONS.into_iter().find(|button| button.id() == id)
  }
}

/// What the buttons show, taken from the player's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shown {
  has_track: bool,
  muted: bool,
  playing: bool,
  repeat: RepeatMode,
}

impl Shown {
  fn of(state: &PlayerState) -> Self {
    Self {
      has_track: state.index.is_some(),
      muted: state.muted,
      playing: state.is_playing,
      repeat: state.repeat,
    }
  }

  fn icon(self, button: Button, icons: &Icons) -> HICON {
    match button {
      Button::Mute if self.muted => icons.muted,
      Button::Mute => icons.sound,
      Button::Previous => icons.previous,
      Button::PlayPause if self.playing => icons.pause,
      Button::PlayPause => icons.play,
      Button::Next => icons.next,
      Button::Repeat => match self.repeat {
        RepeatMode::Off => icons.repeat_off,
        RepeatMode::All => icons.repeat_all,
        RepeatMode::One => icons.repeat_one,
      },
    }
  }

  /// The same words the player bar's hints use.
  fn tip(self, button: Button) -> &'static str {
    match button {
      Button::Mute if self.muted => "Unmute",
      Button::Mute => "Mute",
      Button::Previous => "Previous",
      Button::PlayPause if self.playing => "Pause",
      Button::PlayPause => "Play",
      Button::Next => "Next",
      Button::Repeat => match self.repeat {
        RepeatMode::Off => "Enable repeat",
        RepeatMode::All => "Enable repeat one",
        RepeatMode::One => "Disable repeat",
      },
    }
  }

  fn thumb(self, button: Button, icons: &Icons) -> THUMBBUTTON {
    let mut tip = [0u16; 260];
    for (slot, unit) in tip.iter_mut().zip(self.tip(button).encode_utf16()) {
      *slot = unit;
    }
    // the sound and repeat can be switched with nothing loaded, the rest needs a song
    let enabled = self.has_track || matches!(button, Button::Mute | Button::Repeat);
    THUMBBUTTON {
      dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
      iId: button.id(),
      hIcon: self.icon(button, icons),
      szTip: tip,
      dwFlags: if enabled { THBF_ENABLED } else { THBF_DISABLED },
      ..Default::default()
    }
  }
}

/// The icons of every button state, drawn once for the window's scale.
struct Icons {
  sound: HICON,
  muted: HICON,
  previous: HICON,
  next: HICON,
  play: HICON,
  pause: HICON,
  repeat_off: HICON,
  repeat_all: HICON,
  repeat_one: HICON,
}

impl Icons {
  fn draw(size: u32) -> Option<Self> {
    let icon = |svg: &[u8]| draw_icon(svg, size);
    Some(Self {
      sound: icon(include_bytes!("../../icons/taskbar/sound.svg"))?,
      muted: icon(include_bytes!("../../icons/taskbar/muted.svg"))?,
      previous: icon(include_bytes!("../../icons/taskbar/previous.svg"))?,
      next: icon(include_bytes!("../../icons/taskbar/next.svg"))?,
      play: icon(include_bytes!("../../icons/taskbar/play.svg"))?,
      pause: icon(include_bytes!("../../icons/taskbar/pause.svg"))?,
      repeat_off: icon(include_bytes!("../../icons/taskbar/repeat-off.svg"))?,
      repeat_all: icon(include_bytes!("../../icons/taskbar/repeat-all.svg"))?,
      repeat_one: icon(include_bytes!("../../icons/taskbar/repeat-one.svg"))?,
    })
  }
}

/// Renders a white SVG icon into a square icon handle of `size` pixels.
fn draw_icon(svg: &[u8], size: u32) -> Option<HICON> {
  let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).ok()?;
  let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
  let scale = size as f32 / tree.size().width();
  resvg::render(
    &tree,
    tiny_skia::Transform::from_scale(scale, scale),
    &mut pixmap.as_mut(),
  );
  // the icons are white, so the colour is white wherever anything is drawn and only the alpha varies
  let bgra: Vec<u8> = pixmap
    .pixels()
    .iter()
    .flat_map(|pixel| [255, 255, 255, pixel.alpha()])
    .collect();
  let side = i32::try_from(size).ok()?;
  let info = BITMAPINFO {
    bmiHeader: BITMAPINFOHEADER {
      biSize: u32::try_from(size_of::<BITMAPINFOHEADER>()).ok()?,
      biWidth: side,
      // a negative height stores the rows top to bottom, as the pixmap does
      biHeight: -side,
      biPlanes: 1,
      biBitCount: 32,
      biCompression: BI_RGB.0,
      ..Default::default()
    },
    ..Default::default()
  };
  // SAFETY: the bitmap is created with exactly `bgra.len()` bytes, which are copied into it before any use, and both
  // bitmaps are deleted once the icon holds its own copies
  unsafe {
    let mut bits = std::ptr::null_mut();
    let color = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
    std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits.cast::<u8>(), bgra.len());
    let mask = CreateBitmap(side, side, 1, 1, None);
    let icon = CreateIconIndirect(&ICONINFO {
      fIcon: true.into(),
      hbmMask: mask,
      hbmColor: color,
      ..Default::default()
    });
    delete(color);
    delete(mask);
    icon.ok()
  }
}

fn delete(bitmap: HBITMAP) {
  // SAFETY: the bitmap was created above and is no longer selected anywhere
  let _ = unsafe { DeleteObject(bitmap.into()) };
}

/// The live buttons of the window; shell objects belong to the thread that made them, which is the window's thread.
struct Bar {
  list: ITaskbarList3,
  window: HWND,
  icons: Icons,
  player: Arc<PlayerHandle>,
  /// Set once the taskbar accepted the buttons; they can be added only after the window's taskbar button exists.
  added: bool,
  shown: Shown,
}

impl Bar {
  fn thumbs(&self) -> Vec<THUMBBUTTON> {
    BUTTONS
      .iter()
      .map(|button| self.shown.thumb(*button, &self.icons))
      .collect()
  }

  fn add(&mut self) {
    // SAFETY: the window handle belongs to the live main window and the list was made on this thread
    self.added = unsafe { self.list.ThumbBarAddButtons(self.window, &self.thumbs()) }.is_ok();
  }

  fn show(&mut self, shown: Shown) {
    if shown == self.shown {
      return;
    }
    self.shown = shown;
    if self.added {
      // SAFETY: as in `add`
      let _ = unsafe { self.list.ThumbBarUpdateButtons(self.window, &self.thumbs()) };
    }
  }

  fn press(&self, button: Button) {
    let command = match button {
      Button::Mute => PlayerCommand::SetMuted(!self.player.state().muted),
      Button::Previous => PlayerCommand::Previous,
      Button::PlayPause => PlayerCommand::Toggle,
      Button::Next => PlayerCommand::Next,
      Button::Repeat => PlayerCommand::SetRepeat(self.player.state().repeat.following()),
    };
    self.player.send(command);
  }
}

thread_local! {
  static BAR: RefCell<Option<Bar>> = const { RefCell::new(None) };
}

/// Puts the player buttons into the taskbar preview of `window`; called on the window's thread during setup.
pub fn attach(window: &WebviewWindow, player: Arc<PlayerHandle>) {
  let Ok(raw) = window.hwnd() else { return };
  let hwnd = HWND(raw.0);
  let scale = window.scale_factor().unwrap_or(1.0);
  // rounding keeps the icon a whole number of pixels at any display scale
  #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  let size = (ICON_PX * scale).round() as u32;
  let Some(icons) = Icons::draw(size) else { return };
  // SAFETY: COM is already initialised on the window's thread by the webview
  let Ok(list) = (unsafe { CoCreateInstance::<_, ITaskbarList3>(&TaskbarList, None, CLSCTX_INPROC_SERVER) }) else {
    return;
  };
  // SAFETY: as above
  if unsafe { list.HrInit() }.is_err() {
    return;
  }
  let shown = Shown::of(&player.state());
  let mut bar = Bar {
    list,
    window: hwnd,
    icons,
    player,
    added: false,
    shown,
  };
  // the taskbar button may already be there; otherwise the buttons follow the message announcing it
  bar.add();
  BAR.with(|cell| *cell.borrow_mut() = Some(bar));
  // SAFETY: the procedure only touches the thread-local bar and passes every message on
  let _ = unsafe { SetWindowSubclass(hwnd, Some(on_message), SUBCLASS_ID, 0) };
}

/// Shows the player's new state on the buttons, from whichever thread reports it.
pub fn show(app: &AppHandle, state: &PlayerState) {
  let shown = Shown::of(state);
  let _ = app.run_on_main_thread(move || {
    BAR.with(|cell| {
      if let Some(bar) = cell.borrow_mut().as_mut() {
        bar.show(shown);
      }
    });
  });
}

/// The window message the shell sends once the window's taskbar button exists, and again after Explorer restarts.
fn taskbar_button_created() -> u32 {
  // SAFETY: registering a message name has no preconditions and returns the same number on every call
  unsafe { RegisterWindowMessageW(w!("TaskbarButtonCreated")) }
}

unsafe extern "system" fn on_message(
  window: HWND,
  message: u32,
  wparam: WPARAM,
  lparam: LPARAM,
  _id: usize,
  _data: usize,
) -> LRESULT {
  if message == taskbar_button_created() {
    BAR.with(|cell| {
      if let Some(bar) = cell.borrow_mut().as_mut() {
        bar.add();
      }
    });
  } else if message == WM_COMMAND {
    // the high word tells a thumbnail click apart from other commands, the low word names the button
    let (code, id) = ((wparam.0 >> 16) & 0xffff, wparam.0 & 0xffff);
    let button = u32::try_from(id).ok().and_then(Button::from_id);
    if code == THBN_CLICKED as usize
      && let Some(button) = button
    {
      BAR.with(|cell| {
        if let Some(bar) = cell.borrow().as_ref() {
          bar.press(button);
        }
      });
      return LRESULT(0);
    }
  }
  // SAFETY: the arguments are the ones this window procedure was called with
  unsafe { DefSubclassProc(window, message, wparam, lparam) }
}
