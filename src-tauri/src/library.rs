//! Jellyfin side: folder layout, NFO files, copy to the NAS and library refresh.
//!
//! Layout (Jellyfin "Shows" library):
//!   <library>/<Playlist>/tvshow.nfo, poster.jpg
//!   <library>/<Playlist>/Season 01/<Playlist> - S01E01 - <Title>.mkv / .nfo / -thumb.jpg

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::db::Playlist;
use crate::util::xml_escape;

pub const SEASON_DIR: &str = "Season 01";

pub fn series_dir(library: &str, playlist: &Playlist) -> PathBuf {
    PathBuf::from(library.trim()).join(&playlist.folder)
}

pub fn episode_nfo(info: &Value, title: &str, episode: i64, channel: Option<&str>) -> String {
    let plot = info.get("description").and_then(Value::as_str).unwrap_or("");
    let aired = info
        .get("upload_date")
        .and_then(Value::as_str)
        .filter(|d| d.len() == 8)
        .map(|d| format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8]));
    let runtime = info
        .get("duration")
        .and_then(Value::as_f64)
        .map(|d| (d / 60.0).round() as i64);
    let id = info.get("id").and_then(Value::as_str).unwrap_or("");
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<episodedetails>\n");
    xml += &format!("  <title>{}</title>\n", xml_escape(title));
    xml += &format!("  <plot>{}</plot>\n", xml_escape(plot));
    xml += "  <season>1</season>\n";
    xml += &format!("  <episode>{episode}</episode>\n");
    if let Some(a) = aired {
        xml += &format!("  <aired>{a}</aired>\n  <premiered>{a}</premiered>\n");
    }
    if let Some(r) = runtime {
        xml += &format!("  <runtime>{r}</runtime>\n");
    }
    if let Some(c) = channel {
        xml += &format!("  <studio>{}</studio>\n", xml_escape(c));
    }
    xml += &format!("  <uniqueid type=\"youtube\" default=\"true\">{}</uniqueid>\n", xml_escape(id));
    xml += "  <lockdata>true</lockdata>\n</episodedetails>\n";
    xml
}

fn tvshow_nfo(p: &Playlist) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<tvshow>\n");
    xml += &format!("  <title>{}</title>\n", xml_escape(&p.title));
    xml += &format!("  <plot>{}</plot>\n", xml_escape(p.description.as_deref().unwrap_or("")));
    if let Some(c) = &p.channel {
        xml += &format!("  <studio>{}</studio>\n", xml_escape(c));
    }
    xml += &format!("  <uniqueid type=\"youtube\" default=\"true\">{}</uniqueid>\n", xml_escape(&p.id));
    xml += "  <lockdata>true</lockdata>\n</tvshow>\n";
    xml
}

/// Creates the series folder with its tvshow.nfo and poster if missing.
pub async fn ensure_series(library: &str, playlist: &Playlist) -> Result<PathBuf> {
    let dir = series_dir(library, playlist);
    let season = dir.join(SEASON_DIR);
    tokio::fs::create_dir_all(&season)
        .await
        .with_context(|| format!("Impossible de créer {}", season.display()))?;
    let nfo = dir.join("tvshow.nfo");
    if !nfo.exists() {
        tokio::fs::write(&nfo, tvshow_nfo(playlist)).await?;
    }
    let poster = dir.join("poster.jpg");
    if !poster.exists() {
        if let Some(url) = &playlist.thumbnail {
            // Best effort: a missing poster is not worth failing a download.
            if let Ok(bytes) = fetch_bytes(url).await {
                let _ = tokio::fs::write(&poster, bytes).await;
            }
        }
    }
    Ok(season)
}

async fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
    let resp = reqwest::Client::new()
        .get(url)
        .timeout(Duration::from_secs(30))
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.bytes().await?.to_vec())
}

/// Copies to the library and checks the size before deleting the local file.
pub async fn move_verified(src: &Path, dest: &Path) -> Result<()> {
    let tmp = dest.with_extension("copying");
    tokio::fs::copy(src, &tmp)
        .await
        .with_context(|| format!("Copie vers {} impossible", dest.display()))?;
    let (a, b) = (tokio::fs::metadata(src).await?.len(), tokio::fs::metadata(&tmp).await?.len());
    if a != b {
        let _ = tokio::fs::remove_file(&tmp).await;
        bail!("Copie incomplète vers {}", dest.display());
    }
    tokio::fs::rename(&tmp, dest).await?;
    tokio::fs::remove_file(src).await?;
    Ok(())
}

fn auth_header(key: &str) -> String {
    format!("MediaBrowser Client=\"PlaylistSaver\", Token=\"{}\"", key.trim())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JellyfinInfo {
    pub server_name: String,
    pub version: String,
}

pub async fn jellyfin_test(url: &str, key: &str) -> Result<JellyfinInfo> {
    let base = url.trim().trim_end_matches('/');
    if base.is_empty() || key.trim().is_empty() {
        bail!("Renseigne l'adresse et la clé API Jellyfin.");
    }
    let resp = reqwest::Client::new()
        .get(format!("{base}/System/Info"))
        .header("Authorization", auth_header(key))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .context("Serveur Jellyfin injoignable")?;
    if resp.status().as_u16() == 401 {
        bail!("Clé API refusée par Jellyfin.");
    }
    let v: Value = resp.error_for_status()?.json().await?;
    Ok(JellyfinInfo {
        server_name: v.get("ServerName").and_then(Value::as_str).unwrap_or("Jellyfin").into(),
        version: v.get("Version").and_then(Value::as_str).unwrap_or("").into(),
    })
}

pub async fn jellyfin_refresh(url: &str, key: &str) -> Result<()> {
    let base = url.trim().trim_end_matches('/');
    if base.is_empty() || key.trim().is_empty() {
        return Ok(());
    }
    reqwest::Client::new()
        .post(format!("{base}/Library/Refresh"))
        .header("Authorization", auth_header(key))
        .timeout(Duration::from_secs(15))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
