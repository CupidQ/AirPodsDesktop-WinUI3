//! Global system media transport control (SMTC) for ear-detection pause/play.
//!
//! Ported from `Source/Core/GlobalMedia_win.cpp` — uses
//! `Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager`.

#[cfg(windows)]
pub mod smtc;

#[cfg(windows)]
pub use smtc::{MediaController, PlaybackStatus};

/// Abstract media-control surface used by the state manager.
pub trait MediaControl: Send + Sync {
    fn pause(&self);
    fn play(&self);
    fn toggle(&self);
    fn is_playing(&self) -> bool;
}

/// No-op controller (used when media control is disabled).
#[derive(Debug, Default, Clone, Copy)]
pub struct NullMediaControl;

impl MediaControl for NullMediaControl {
    fn pause(&self) {}
    fn play(&self) {}
    fn toggle(&self) {}
    fn is_playing(&self) -> bool {
        false
    }
}
