//! Everything that talks to yt-dlp: reading playlists, picking formats, downloading.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio_util::sync::CancellationToken;

use crate::tools::Tools;
use crate::util::{cmd, kill_tree, last_error, Stopped};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEntry {
    pub id: String,
    pub title: String,
    pub channel: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<f64>,
}

/// What a pasted link points to: a playlist (or channel tab) or a single video.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    /// "playlist" or "video".
    pub kind: String,
    pub id: String,
    pub title: String,
    pub channel: Option<String>,
    pub url: String,
    pub thumbnail: Option<String>,
    pub description: Option<String>,
    pub entries: Vec<RemoteEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub id: i64,
    pub percent: f64,
    pub speed: Option<f64>,
    pub eta: Option<f64>,
    pub stage: String,
}

pub fn emit_progress(app: &AppHandle, id: i64, percent: f64, stage: &str) {
    let _ = app.emit(
        "video-progress",
        Progress { id, percent, speed: None, eta: None, stage: stage.into() },
    );
}

fn base_args(tools: &Tools) -> Vec<String> {
    vec![
        "--ignore-config".into(),
        "--color".into(),
        "never".into(),
        "--encoding".into(),
        "utf-8".into(),
        "--js-runtimes".into(),
        format!("deno:{}", tools.deno.display()),
        "--ffmpeg-location".into(),
        tools.bin_dir.display().to_string(),
    ]
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn video_thumb(id: &str) -> String {
    format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")
}

/// Reads stdout/stderr line by line, tolerating invalid UTF-8.
async fn read_lines<R: AsyncRead + Unpin>(reader: R, mut f: impl FnMut(String)) {
    let mut split = BufReader::new(reader).split(b'\n');
    while let Ok(Some(bytes)) = split.next_segment().await {
        f(String::from_utf8_lossy(&bytes).trim_end_matches('\r').to_string());
    }
}

/// Runs yt-dlp and parses its JSON output. Can be cancelled.
async fn run_json(tools: &Tools, args: &[String], cancel: Option<&CancellationToken>) -> Result<Value> {
    let mut child = cmd(&tools.ytdlp)
        .args(base_args(tools))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let pid = child.id().unwrap_or(0);
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let out_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        let mut reader = BufReader::new(stdout);
        tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut buf).await.ok();
        buf
    });
    let err_task = tokio::spawn(async move {
        let mut lines = Vec::new();
        read_lines(stderr, |l| lines.push(l)).await;
        lines
    });
    let status = match cancel {
        Some(token) => tokio::select! {
            _ = token.cancelled() => {
                kill_tree(pid);
                let _ = child.wait().await;
                return Err(Stopped.into());
            }
            s = child.wait() => s?,
        },
        None => child.wait().await?,
    };
    let stdout = out_task.await.unwrap_or_default();
    let stderr = err_task.await.unwrap_or_default();
    if !status.success() {
        bail!("{}", last_error(&stderr));
    }
    Ok(serde_json::from_slice(&stdout)?)
}

fn entry_from(v: &Value) -> Option<RemoteEntry> {
    let id = str_of(v, "id")?;
    // Channel pages also list tabs / sub-playlists, real videos have 11-char ids.
    if id.len() != 11 {
        return None;
    }
    let title = str_of(v, "title").unwrap_or_else(|| id.clone());
    if title.starts_with("[Private") || title.starts_with("[Deleted") {
        return None;
    }
    Some(RemoteEntry {
        thumbnail: Some(video_thumb(&id)),
        channel: str_of(v, "channel").or_else(|| str_of(v, "uploader")),
        duration: v.get("duration").and_then(Value::as_f64),
        title,
        id,
    })
}

fn best_thumbnail(v: &Value) -> Option<String> {
    let thumbs = v.get("thumbnails")?.as_array()?;
    let area = |t: &Value| {
        t.get("width").and_then(Value::as_f64).unwrap_or(0.0)
            * t.get("height").and_then(Value::as_f64).unwrap_or(0.0)
    };
    let pick = |jpg_only: bool| {
        thumbs
            .iter()
            .filter(|t| !jpg_only || str_of(t, "url").is_some_and(|u| u.contains(".jpg")))
            .max_by(|a, b| area(a).total_cmp(&area(b)))
            .and_then(|t| str_of(t, "url"))
    };
    pick(true).or_else(|| pick(false))
}

/// Lists a playlist (flat, fast) or reads a single video.
pub async fn fetch_remote(tools: &Tools, url: &str) -> Result<RemoteInfo> {
    let v = run_json(
        tools,
        &["-J".into(), "--flat-playlist".into(), url.to_string()],
        None,
    )
    .await?;
    if v.get("_type").and_then(Value::as_str) == Some("playlist") {
        let entries: Vec<RemoteEntry> = v
            .get("entries")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(entry_from).collect())
            .unwrap_or_default();
        if entries.is_empty() {
            bail!("Aucune vidéo disponible dans cette playlist.");
        }
        Ok(RemoteInfo {
            kind: "playlist".into(),
            id: str_of(&v, "id").unwrap_or_default(),
            title: str_of(&v, "title").unwrap_or_else(|| "Playlist".into()),
            channel: str_of(&v, "channel").or_else(|| str_of(&v, "uploader")),
            url: str_of(&v, "webpage_url").unwrap_or_else(|| url.to_string()),
            thumbnail: best_thumbnail(&v).or_else(|| entries[0].thumbnail.clone()),
            description: str_of(&v, "description"),
            entries,
        })
    } else {
        let Some(entry) = entry_from(&v) else {
            bail!("Ce lien ne correspond pas à une vidéo YouTube.");
        };
        Ok(RemoteInfo {
            kind: "video".into(),
            id: entry.id.clone(),
            title: entry.title.clone(),
            channel: entry.channel.clone(),
            url: str_of(&v, "webpage_url").unwrap_or_else(|| url.to_string()),
            thumbnail: entry.thumbnail.clone(),
            description: str_of(&v, "description"),
            entries: vec![entry],
        })
    }
}

/// Full metadata of one video, including every available format.
pub async fn fetch_video(tools: &Tools, yt_id: &str, cancel: &CancellationToken) -> Result<Value> {
    run_json(
        tools,
        &[
            "-J".into(),
            "--no-playlist".into(),
            format!("https://www.youtube.com/watch?v={yt_id}"),
        ],
        Some(cancel),
    )
    .await
}

#[derive(Debug, Clone)]
pub struct AudioPick {
    pub format_id: String,
    /// ISO 639-2 code for the MKV track.
    pub language: Option<String>,
    pub label: String,
    pub french: bool,
}

#[derive(Debug, Clone)]
pub struct Selection {
    /// Value for yt-dlp `-f`.
    pub format: String,
    pub video_id: Option<String>,
    /// Audio tracks in the order they end up in the MKV.
    pub audios: Vec<AudioPick>,
}

fn lang_name(code: &str) -> String {
    let short = code.split(['-', '_']).next().unwrap_or(code).to_lowercase();
    match short.as_str() {
        "fr" => "Français",
        "en" => "Anglais",
        "es" => "Espagnol",
        "de" => "Allemand",
        "it" => "Italien",
        "pt" => "Portugais",
        "ja" => "Japonais",
        "ko" => "Coréen",
        "zh" => "Chinois",
        "ru" => "Russe",
        "ar" => "Arabe",
        "hi" => "Hindi",
        "nl" => "Néerlandais",
        "pl" => "Polonais",
        "tr" => "Turc",
        "id" => "Indonésien",
        _ => return short.to_uppercase(),
    }
    .into()
}

pub fn iso639_2(code: &str) -> String {
    let short = code.split(['-', '_']).next().unwrap_or(code).to_lowercase();
    match short.as_str() {
        "fr" => "fre",
        "en" => "eng",
        "es" => "spa",
        "de" => "ger",
        "it" => "ita",
        "pt" => "por",
        "ja" => "jpn",
        "ko" => "kor",
        "zh" => "chi",
        "ru" => "rus",
        "ar" => "ara",
        "hi" => "hin",
        "nl" => "dut",
        "pl" => "pol",
        "tr" => "tur",
        "id" => "ind",
        _ => return short,
    }
    .into()
}

/// Picks the best video stream and the French + original audio tracks.
pub fn select_formats(info: &Value, max_height: u32, vf_first: bool) -> Selection {
    let formats: Vec<&Value> = info
        .get("formats")
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let codec = |f: &Value, k: &str| f.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let num = |f: &Value, k: &str| f.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let https = |f: &Value| codec(f, "protocol").starts_with("http") as u8 as f64;

    let video = formats
        .iter()
        .filter(|f| {
            let v = codec(f, "vcodec");
            !v.is_empty() && v != "none" && codec(f, "acodec") == "none"
        })
        .filter(|f| max_height == 0 || num(f, "height") <= max_height as f64)
        .max_by(|a, b| {
            (num(a, "height"), https(a), num(a, "fps"), num(a, "tbr"))
                .partial_cmp(&(num(b, "height"), https(b), num(b, "fps"), num(b, "tbr")))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .and_then(|f| str_of(f, "format_id"));

    let audio: Vec<&Value> = formats
        .iter()
        .copied()
        .filter(|f| {
            let a = codec(f, "acodec");
            !a.is_empty() && a != "none" && codec(f, "vcodec") == "none"
        })
        .filter(|f| !codec(f, "format_id").contains("drc"))
        // Audio description tracks.
        .filter(|f| f.get("language_preference").and_then(Value::as_i64) != Some(-10))
        .collect();
    let lang = |f: &Value| codec(f, "language").to_lowercase();
    let best = |pred: &dyn Fn(&Value) -> bool| {
        audio
            .iter()
            .copied()
            .filter(|f| pred(f))
            .max_by(|a, b| {
                (https(a), num(a, "abr").max(num(a, "tbr")))
                    .partial_cmp(&(https(b), num(b, "abr").max(num(b, "tbr"))))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    };
    let original_lang = str_of(info, "language").unwrap_or_default().to_lowercase();
    let original = best(&|f| {
        codec(f, "format_note").to_lowercase().contains("original")
            || f.get("language_preference").and_then(Value::as_i64) == Some(10)
    })
    .or_else(|| best(&|f| !original_lang.is_empty() && lang(f) == original_lang))
    .or_else(|| best(&|_| true));
    let french = best(&|f| lang(f).starts_with("fr"));

    let pick = |f: &Value, is_original: bool| {
        let l = lang(f);
        let l = if l.is_empty() { original_lang.clone() } else { l };
        let fr = l.starts_with("fr");
        AudioPick {
            format_id: codec(f, "format_id"),
            language: (!l.is_empty()).then(|| iso639_2(&l)),
            label: match (fr, is_original) {
                (true, true) => "Français".into(),
                (true, false) => "Français (VF)".into(),
                (false, _) if l.is_empty() => "Version originale".into(),
                (false, _) => format!("{} (VO)", lang_name(&l)),
            },
            french: fr,
        }
    };

    let mut audios = Vec::new();
    if let Some(o) = original {
        let o_pick = pick(o, true);
        match french {
            Some(f) if !o_pick.french && codec(f, "format_id") != o_pick.format_id => {
                let f_pick = pick(f, false);
                if vf_first {
                    audios = vec![f_pick, o_pick];
                } else {
                    audios = vec![o_pick, f_pick];
                }
            }
            _ => audios.push(o_pick),
        }
    }

    match (&video, audios.is_empty()) {
        (Some(v), false) => Selection {
            format: std::iter::once(v.clone())
                .chain(audios.iter().map(|a| a.format_id.clone()))
                .collect::<Vec<_>>()
                .join("+"),
            video_id: Some(v.clone()),
            audios,
        },
        // Odd video without separate streams: let yt-dlp decide.
        _ => Selection {
            format: if max_height > 0 {
                format!("bv*[height<={max_height}]+ba/b[height<={max_height}]/bv*+ba/b")
            } else {
                "bv*+ba/b".into()
            },
            video_id: None,
            audios: Vec::new(),
        },
    }
}

pub struct DownloadJob<'a> {
    pub video_id: i64,
    pub info_json: &'a Path,
    pub selection: &'a Selection,
    pub workdir: &'a Path,
    pub base: &'a str,
    pub auto_subs: bool,
    pub rate_limit: &'a str,
}

fn parse_num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok()
}

/// Downloads and merges everything into `<workdir>/<base>.mkv` (+ thumbnail).
pub async fn download(
    app: &AppHandle,
    tools: &Tools,
    job: &DownloadJob<'_>,
    cancel: &CancellationToken,
) -> Result<PathBuf> {
    let out = job.workdir.join(format!("{}.%(ext)s", job.base));
    let thumb = job.workdir.join(format!("{}-thumb.%(ext)s", job.base));
    let mut args: Vec<String> = vec![
        "--load-info-json".into(),
        job.info_json.display().to_string(),
        "-f".into(),
        job.selection.format.clone(),
        "--audio-multistreams".into(),
        "--merge-output-format".into(),
        "mkv".into(),
        "--remux-video".into(),
        "mkv".into(),
        "--write-subs".into(),
        "--sub-langs".into(),
        "fr.*,-fr-orig,-live_chat".into(),
        "--convert-subs".into(),
        "srt".into(),
        "--embed-subs".into(),
        "--embed-metadata".into(),
        "--embed-chapters".into(),
        "--write-thumbnail".into(),
        "--convert-thumbnails".into(),
        "jpg".into(),
        "--no-overwrites".into(),
        "--newline".into(),
        "--progress-template".into(),
        "download:PS|%(info.format_id)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s".into(),
        "-o".into(),
        out.display().to_string(),
        "-o".into(),
        format!("thumbnail:{}", thumb.display()),
    ];
    if job.auto_subs {
        args.push("--write-auto-subs".into());
    }
    if !job.rate_limit.trim().is_empty() {
        args.push("--limit-rate".into());
        args.push(job.rate_limit.trim().to_string());
    }

    // Stage list used to turn per-stream progress into one global percentage.
    let mut stages: Vec<(String, String)> = Vec::new();
    if let Some(v) = &job.selection.video_id {
        stages.push((v.clone(), "Vidéo".into()));
    }
    for a in &job.selection.audios {
        stages.push((a.format_id.clone(), format!("Audio {}", a.label)));
    }
    let stage_count = stages.len().max(1) as f64;

    let mut child = cmd(&tools.ytdlp)
        .args(base_args(tools))
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let pid = child.id().unwrap_or(0);
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let err_task = tokio::spawn(async move {
        let mut lines: Vec<String> = Vec::new();
        read_lines(stderr, |l| {
            lines.push(l);
            if lines.len() > 80 {
                lines.remove(0);
            }
        })
        .await;
        lines
    });

    let video_id = job.video_id;
    let app_out = app.clone();
    let out_task = tokio::spawn(async move {
        let mut last_emit = Instant::now() - Duration::from_secs(1);
        read_lines(stdout, |line| {
            let progress = if let Some(rest) = line.strip_prefix("PS|") {
                let parts: Vec<&str> = rest.split('|').collect();
                if parts.len() < 6 {
                    return;
                }
                let Some(idx) = stages.iter().position(|(id, _)| id == parts[0]) else {
                    return;
                };
                let done = parse_num(parts[1]).unwrap_or(0.0);
                let total = parse_num(parts[2]).or_else(|| parse_num(parts[3])).unwrap_or(0.0);
                let frac = if total > 0.0 { (done / total).min(1.0) } else { 0.0 };
                let is_last = frac >= 1.0;
                if !is_last && last_emit.elapsed() < Duration::from_millis(300) {
                    return;
                }
                Progress {
                    id: video_id,
                    percent: (idx as f64 + frac) / stage_count * 95.0,
                    speed: parse_num(parts[4]),
                    eta: parse_num(parts[5]),
                    stage: stages[idx].1.clone(),
                }
            } else if line.starts_with("[Merger]")
                || line.starts_with("[EmbedSubtitle]")
                || line.starts_with("[VideoRemuxer]")
                || line.starts_with("[Metadata]")
            {
                Progress {
                    id: video_id,
                    percent: 96.0,
                    speed: None,
                    eta: None,
                    stage: "Assemblage".into(),
                }
            } else {
                return;
            };
            last_emit = Instant::now();
            let _ = app_out.emit("video-progress", progress);
        })
        .await;
    });

    let status = tokio::select! {
        _ = cancel.cancelled() => {
            kill_tree(pid);
            let _ = child.wait().await;
            out_task.abort();
            return Err(Stopped.into());
        }
        s = child.wait() => s?,
    };
    let _ = out_task.await;
    let stderr = err_task.await.unwrap_or_default();
    if !status.success() {
        bail!("{}", last_error(&stderr));
    }
    let file = job.workdir.join(format!("{}.mkv", job.base));
    if !file.exists() {
        bail!("Fichier final introuvable après le téléchargement.");
    }
    Ok(file)
}

/// Names the audio tracks, sets which one plays by default and labels subtitles.
pub async fn tag_tracks(tools: &Tools, file: &Path, selection: &Selection) -> Result<()> {
    if selection.audios.is_empty() {
        return Ok(());
    }
    let probe = cmd(&tools.ffprobe)
        .args(["-v", "error", "-select_streams", "s", "-show_entries", "stream=index", "-of", "csv=p=0"])
        .arg(file)
        .output()
        .await?;
    let sub_count = String::from_utf8_lossy(&probe.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();

    let tagged = file.with_extension("tagged.mkv");
    let mut c = cmd(&tools.ffmpeg);
    c.args(["-y", "-v", "error", "-i"]).arg(file).args(["-map", "0", "-c", "copy"]);
    for (i, a) in selection.audios.iter().enumerate() {
        if let Some(lang) = &a.language {
            c.arg(format!("-metadata:s:a:{i}")).arg(format!("language={lang}"));
        }
        c.arg(format!("-metadata:s:a:{i}")).arg(format!("title={}", a.label));
        c.arg(format!("-disposition:a:{i}")).arg(if i == 0 { "default" } else { "0" });
    }
    // French subtitles shown by default only when the default audio is not French.
    let first_is_french = selection.audios[0].french;
    for i in 0..sub_count {
        c.arg(format!("-metadata:s:s:{i}")).arg("language=fre");
        c.arg(format!("-metadata:s:s:{i}")).arg("title=Français");
        c.arg(format!("-disposition:s:{i}"))
            .arg(if i == 0 && !first_is_french { "default" } else { "0" });
    }
    c.arg(&tagged);
    let out = c.output().await?;
    if !out.status.success() || !tagged.exists() {
        let _ = tokio::fs::remove_file(&tagged).await;
        bail!(
            "ffmpeg: {}",
            String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("")
        );
    }
    tokio::fs::rename(&tagged, file).await?;
    Ok(())
}
