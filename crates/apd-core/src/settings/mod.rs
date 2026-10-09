//! Persistent application settings.
//!
//! Stored as JSON in `%APPDATA%\AirPodsDesktop\settings.json`, matching the
//! spirit of the original `Core::Settings` module.

use std::fs;
use std::path::PathBuf;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Only accept advertisements with RSSI ≥ this value.
    pub rssi_min: i16,
    /// Bound Bluetooth address (0 = auto / any).
    pub bound_device_address: u64,
    /// Friendly name override for the bound device.
    pub bound_device_name: String,
    /// Pause media when both pods leave the ear / resume when inserted.
    pub automatic_ear_detection: bool,
    /// Enable low-latency audio profile switching (best-effort).
    pub low_audio_latency: bool,
    /// Launch at user login.
    pub auto_start: bool,
    /// UI theme: "system" | "light" | "dark"
    pub theme: String,
    /// Language tag, e.g. "en-US", "zh-CN".
    pub language: String,
    /// Show battery popup when lid is opened / device connected.
    pub show_popup_on_connect: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            rssi_min: -80,
            bound_device_address: 0,
            bound_device_name: String::new(),
            automatic_ear_detection: true,
            low_audio_latency: false,
            auto_start: false,
            theme: "system".into(),
            language: String::new(),
            show_popup_on_connect: true,
        }
    }
}

impl Settings {
    pub fn config_dir() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|appdata| PathBuf::from(appdata).join("AirPodsDesktop"))
    }

    pub fn config_path() -> Option<PathBuf> {
        Self::config_dir().map(|d| d.join("settings.json"))
    }

    /// Load settings from disk, or defaults when missing/corrupt.
    pub fn load() -> Settings {
        match Self::config_path() {
            Some(path) if path.exists() => fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            _ => Settings::default(),
        }
    }

    /// Save settings to disk. Creates the config directory when needed.
    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "APPDATA not available")
        })?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, json)
    }
}

/// Shared live settings handle.
#[derive(Clone)]
pub struct SettingsStore {
    inner: std::sync::Arc<Mutex<Settings>>,
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsStore {
    pub fn new() -> Self {
        SettingsStore {
            inner: std::sync::Arc::new(Mutex::new(Settings::load())),
        }
    }

    pub fn get(&self) -> Settings {
        self.inner.lock().clone()
    }

    pub fn update<F: FnOnce(&mut Settings)>(&self, f: F) -> Settings {
        let mut g = self.inner.lock();
        f(&mut g);
        let snapshot = g.clone();
        let _ = snapshot.save();
        snapshot
    }

    pub fn replace(&self, settings: Settings) {
        let _ = settings.save();
        *self.inner.lock() = settings;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings() {
        let s = Settings::default();
        assert_eq!(s.rssi_min, -80);
        assert!(s.automatic_ear_detection);
        assert_eq!(s.theme, "system");
    }

    #[test]
    fn serde_roundtrip() {
        let mut s = Settings::default();
        s.rssi_min = -60;
        s.theme = "dark".into();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
