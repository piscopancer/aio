use std::sync::Arc;

use super::error::AppResult;
use super::ports::PinStore;
use crate::domain::pins::{PinTarget, Pins};

/// Pinning, each change a load → domain action → save round trip on the pins of the target's kind.
pub struct PinService {
  pins: Arc<dyn PinStore>,
}

impl PinService {
  pub fn new(pins: Arc<dyn PinStore>) -> Self {
    Self { pins }
  }

  pub fn pin(&self, target: PinTarget) -> AppResult<()> {
    self.change(target, |pins| {
      pins.pin(target.raw());
      Ok(())
    })
  }

  pub fn unpin(&self, target: PinTarget) -> AppResult<()> {
    self.change(target, |pins| {
      pins.unpin(target.raw());
      Ok(())
    })
  }

  /// Moves a pinned item to place `to` among the pinned of its kind.
  pub fn move_pin(&self, target: PinTarget, to: u32) -> AppResult<()> {
    self.change(target, |pins| Ok(pins.move_to(target.raw(), to as usize)?))
  }

  fn change(&self, target: PinTarget, action: impl FnOnce(&mut Pins) -> AppResult<()>) -> AppResult<()> {
    let kind = target.kind();
    let mut pins = self.pins.load_pins(kind)?;
    action(&mut pins)?;
    self.pins.save_pins(kind, &pins)
  }
}
