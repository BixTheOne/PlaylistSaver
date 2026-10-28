//! External binaries: yt-dlp, ffmpeg/ffprobe and deno (JS runtime yt-dlp needs for YouTube).
//! They are downloaded on first launch into the app data folder.

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

use crate::util::cmd;

const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const FFMPEG_URL: &str =
    "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";
const DENO_URL: &str =
    "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip";

pub struct Tools {
    pub bin_dir: PathBuf,
    pub ytdlp: PathBuf,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub deno: PathBuf,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolsProgress<'a> {
    tool: &'a str,
    downloaded: u64,
    total: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsVersions {
    pub ytdlp: Option<String>,
    pub ffmpeg: Option<String>,
    pub deno: Option<String>,
}

impl Tools {
    pub fn new(bin_dir: PathBuf) -> Self {
        Self {
            ytdlp: bin_dir.join("yt-dlp.exe"),
            ffmpeg: bin_dir.join("ffmpeg.exe"),
            ffprobe: bin_dir.join("ffprobe.exe"),
            deno: bin_dir.join("deno.exe"),
            bin_dir,
        }
    }

    pub fn ready(&self) -> bool {
        [&self.ytdlp, &self.ffmpeg, &self.ffprobe, &self.deno]
            .iter()
            .all(|p| p.exists())
    }

    pub async fn ensure(&self, app: &AppHandle) -> Result<()> {
        tokio::fs::create_dir_all(&self.bin_dir).await?;
        if !self.ytdlp.exists() {
            download(app, "yt-dlp", YTDLP_URL, &self.ytdlp).await?;
        }
        if !self.ffmpeg.exists() || !self.ffprobe.exists() {
            let zip = self.bin_dir.join("ffmpeg.zip");
            download(app, "ffmpeg", FFMPEG_URL, &zip).await?;
            extract(
                zip.clone(),
                vec![
                    ("ffmpeg.exe", self.ffmpeg.clone()),
                    ("ffprobe.exe", self.ffprobe.clone()),
                ],
            )
            .await?;
            let _ = tokio::fs::remove_file(&zip).await;
        }
        if !self.deno.exists() {
            let zip = self.bin_dir.join("deno.zip");
            download(app, "deno", DENO_URL, &zip).await?;
            extract(zip.clone(), vec![("deno.exe", self.deno.clone())]).await?;
            let _ = tokio::fs::remove_file(&zip).await;
        }
        Ok(())
    }

    pub async fn versions(&self) -> ToolsVersions {
        ToolsVersions {
            ytdlp: first_line(&self.ytdlp, &["--version"]).await,
            ffmpeg: first_line(&self.ffmpeg, &["-version"]).await.map(|l| {
                l.split_whitespace().nth(2).unwrap_or(&l).to_string()
            }),
            deno: first_line(&self.deno, &["--version"]).await.map(|l| {
                l.split_whitespace().nth(1).unwrap_or(&l).to_string()
            }),
        }
    }

    /// Self-update of yt-dlp, YouTube changes often break old versions.
    pub async fn update_ytdlp(&self) -> Result<String> {
        let out = cmd(&self.ytdlp).arg("-U").output().await?;
        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        let last = text
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_string();
        if !out.status.success() {
            bail!("{last}");
        }
        Ok(last)
    }
}

async fn first_line(path: &Path, args: &[&str]) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let out = cmd(path).args(args).output().await.ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .map(|l| l.trim().to_string())
}

async fn download(app: &AppHandle, tool: &str, url: &str, dest: &Path) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("PlaylistSaver")
        .build()?;
    let resp = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .with_context(|| format!("Téléchargement de {tool} impossible"))?;
    let total = resp.content_length();
    let part = dest.with_extension("part");
    let mut file = tokio::fs::File::create(&part).await?;
    let mut stream = resp.bytes_stream();
    let mut downloaded = 0u64;
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("Téléchargement de {tool} interrompu"))?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        if last_emit.elapsed() > Duration::from_millis(200) {
            last_emit = Instant::now();
            let _ = app.emit("tools-progress", ToolsProgress { tool, downloaded, total });
        }
    }
    file.flush().await?;
    drop(file);
    tokio::fs::rename(&part, dest).await?;
    let _ = app.emit("tools-progress", ToolsProgress { tool, downloaded, total: Some(downloaded) });
    Ok(())
}

/// Pulls the wanted files out of a zip, wherever they are inside it.
async fn extract(zip_path: PathBuf, wanted: Vec<(&'static str, PathBuf)>) -> Result<()> {
    tokio::task::spawn_blocking(move || -> Result<()> {
        let file = std::fs::File::open(&zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name()?.replace('\\', "/");
            let file_name = name.rsplit('/').next().unwrap_or("").to_string();
            if let Some((_, dest)) = wanted
                .iter()
                .find(|(w, _)| file_name.eq_ignore_ascii_case(w))
            {
                let mut out = std::fs::File::create(dest)?;
                std::io::copy(&mut entry, &mut out)?;
            }
        }
        for (name, dest) in &wanted {
            if !dest.exists() {
                bail!("{name} introuvable dans l'archive");
            }
        }
        Ok(())
    })
    .await?
}
