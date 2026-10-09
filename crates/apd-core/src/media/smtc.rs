//! Windows SMTC (System Media Transport Controls) integration.

use parking_lot::Mutex;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};

use crate::media::MediaControl;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Closed,
    Opened,
    Changing,
    Stopped,
    Playing,
    Paused,
}

impl From<GlobalSystemMediaTransportControlsSessionPlaybackStatus> for PlaybackStatus {
    fn from(v: GlobalSystemMediaTransportControlsSessionPlaybackStatus) -> Self {
        match v {
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => {
                PlaybackStatus::Playing
            }
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused => {
                PlaybackStatus::Paused
            }
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped => {
                PlaybackStatus::Stopped
            }
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Opened => {
                PlaybackStatus::Opened
            }
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Changing => {
                PlaybackStatus::Changing
            }
            _ => PlaybackStatus::Closed,
        }
    }
}

/// Wrapper around the current SMTC session.
pub struct MediaController {
    manager: Mutex<Option<GlobalSystemMediaTransportControlsSessionManager>>,
}

impl MediaController {
    pub fn new() -> windows::core::Result<Self> {
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .and_then(|op| op.get())?;
        Ok(MediaController {
            manager: Mutex::new(Some(manager)),
        })
    }

    fn with_session<R>(
        &self,
        f: impl FnOnce(&GlobalSystemMediaTransportControlsSession) -> windows::core::Result<R>,
    ) -> windows::core::Result<R> {
        let guard = self.manager.lock();
        let manager = guard
            .as_ref()
            .ok_or_else(|| windows::core::Error::from(windows::core::HRESULT(0x80004005u32 as i32)))?;
        let session = manager.GetCurrentSession()?;
        f(&session)
    }

    pub fn playback_status(&self) -> Option<PlaybackStatus> {
        self.with_session(|s| s.GetPlaybackInfo()?.PlaybackStatus())
            .ok()
            .map(PlaybackStatus::from)
    }

    pub fn pause_blocking(&self) -> windows::core::Result<bool> {
        self.with_session(|s| s.TryPauseAsync())?.get()
    }

    pub fn play_blocking(&self) -> windows::core::Result<bool> {
        self.with_session(|s| s.TryPlayAsync())?.get()
    }

    pub fn toggle_blocking(&self) -> windows::core::Result<bool> {
        match self.playback_status() {
            Some(PlaybackStatus::Playing) => self.pause_blocking(),
            _ => self.play_blocking(),
        }
    }
}

impl MediaControl for MediaController {
    fn pause(&self) {
        if let Err(error) = self.pause_blocking() {
            log::warn!("SMTC pause failed: {error}");
        }
    }

    fn play(&self) {
        if let Err(error) = self.play_blocking() {
            log::warn!("SMTC play failed: {error}");
        }
    }

    fn toggle(&self) {
        if let Err(error) = self.toggle_blocking() {
            log::warn!("SMTC toggle failed: {error}");
        }
    }

    fn is_playing(&self) -> bool {
        self.playback_status() == Some(PlaybackStatus::Playing)
    }
}
