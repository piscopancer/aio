//! The main window's life apart from the webview: brought back from the tray or by a second launch, and hidden
//! instead of closed while the user keeps the app in the tray.

use tauri::{AppHandle, Manager, Window, WindowEvent};

use crate::application::ports::WindowSettingsStore;

/// Label of the one window, as the Tauri config names it.
pub const MAIN: &str = "main";

/// Shows the window, restores it when minimised and brings it to the front.
pub fn reveal(app: &AppHandle) {
  let Some(window) = app.get_webview_window(MAIN) else {
    return;
  };
  let _ = window.show();
  let _ = window.unminimize();
  let _ = window.set_focus();
}

/// Hides the window to the tray when it is asked to close while the user keeps the app there; the tray's Quit ends it.
pub fn on_event(window: &Window, event: &WindowEvent, settings: &dyn WindowSettingsStore) {
  if let WindowEvent::CloseRequested { api, .. } = event
    && settings.window_settings().close_to_tray
  {
    api.prevent_close();
    let _ = window.hide();
  }
}
