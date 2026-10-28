use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Jellyfin library root (usually a NAS share like \\NAS\media\YouTube).
    pub library_dir: String,
    /// Local working folder. Empty = app data folder.
    pub temp_dir: String,
    /// 0 = best available.
    pub max_height: u32,
    /// Use YouTube auto-generated / auto-translated French subtitles when no manual ones exist.
    pub auto_subs: bool,
    /// "vf" or "vo": which audio track is the default one in the MKV.
    pub default_audio: String,
    /// yt-dlp --limit-rate value, e.g. "5M". Empty = unlimited.
    pub rate_limit: String,
    pub concurrency: u32,
    /// 0 = automatic sync disabled.
    pub sync_interval_hours: u32,
    pub singles_title: String,
    pub jellyfin_url: String,
    pub jellyfin_api_key: String,
    /// "light", "dark" or "system".
    pub theme: String,
    pub close_to_tray: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            library_dir: String::new(),
            temp_dir: String::new(),
            max_height: 0,
            auto_subs: true,
            default_audio: "vf".into(),
            rate_limit: String::new(),
            concurrency: 1,
            sync_interval_hours: 6,
            singles_title: "Vidéos YouTube".into(),
            jellyfin_url: String::new(),
            jellyfin_api_key: String::new(),
            theme: "system".into(),
            close_to_tray: true,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
