//! Download queue: a worker picks queued videos and runs the full pipeline
//! (metadata, download, track tagging, NFO, copy to the library).

use anyhow::{bail, Result};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use crate::db::{self, Video};
use crate::library;
use crate::state::{AppState, Running, StopReason};
use crate::util::{sanitize, Stopped};
use crate::ytdlp::{self, emit_progress, DownloadJob};

pub fn notify_changed(app: &AppHandle) {
    let _ = app.emit("videos-changed", ());
}

pub fn start(app: AppHandle, state: Arc<AppState>) {
    if let Err(e) = db::reset_downloading(&state.db()) {
        eprintln!("reset queue: {e:#}");
    }
    tauri::async_runtime::spawn(async move {
        loop {
            fill_slots(&app, &state);
            tokio::select! {
                _ = state.queue.notify.notified() => {}
                _ = tokio::time::sleep(Duration::from_secs(5)) => {}
            }
        }
    });
}

fn fill_slots(app: &AppHandle, state: &Arc<AppState>) {
    if !state.tools.ready() {
        return;
    }
    loop {
        if state.queue.paused.load(Ordering::SeqCst) {
            return;
        }
        let limit = state.settings().concurrency.clamp(1, 4) as usize;
        let running = state.queue.running.lock().unwrap_or_else(|e| e.into_inner()).len();
        if running >= limit {
            return;
        }
        let next = {
            let conn = state.db();
            match db::next_queued(&conn) {
                Ok(Some(v)) => {
                    let _ = db::set_status(&conn, v.id, "downloading", None);
                    v
                }
                _ => return,
            }
        };
        spawn_job(app.clone(), state.clone(), next);
    }
}

fn spawn_job(app: AppHandle, state: Arc<AppState>, video: Video) {
    let token = CancellationToken::new();
    let reason = Arc::new(Mutex::new(StopReason::Requeue));
    state
        .queue
        .running
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(video.id, Running { token: token.clone(), reason: reason.clone() });
    notify_changed(&app);

    tauri::async_runtime::spawn(async move {
        let result = process(&app, &state, &video, &token).await;
        state
            .queue
            .running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&video.id);
        let stopped = token.is_cancelled()
            || result.as_ref().err().is_some_and(|e| e.downcast_ref::<Stopped>().is_some());
        let mut finished_ok = false;
        {
            let conn = state.db();
            let _ = match &result {
                Ok(path) => {
                    finished_ok = true;
                    db::set_done(&conn, video.id, path)
                }
                Err(_) if stopped => match *reason.lock().unwrap_or_else(|e| e.into_inner()) {
                    StopReason::Pause => db::set_status(&conn, video.id, "paused", None),
                    StopReason::Requeue => db::set_status(&conn, video.id, "queued", None),
                    StopReason::Remove => db::set_status(&conn, video.id, "new", None),
                },
                Err(e) => db::set_status(&conn, video.id, "error", Some(&format!("{e:#}"))),
            };
        }
        if stopped && *reason.lock().unwrap_or_else(|e| e.into_inner()) == StopReason::Remove {
            cleanup_partial(&state, &video).await;
        }
        notify_changed(&app);
        if finished_ok && !db::has_pending(&state.db()).unwrap_or(true) {
            let s = state.settings();
            if let Err(e) = library::jellyfin_refresh(&s.jellyfin_url, &s.jellyfin_api_key).await {
                eprintln!("jellyfin refresh: {e:#}");
            }
        }
        state.queue.notify.notify_one();
    });
}

fn episode_prefix(folder: &str, episode: i64) -> String {
    format!("{} - S01E{:02} - ", sanitize(folder, 60), episode)
}

/// Removes leftovers (.part, fragments) of a cancelled download.
async fn cleanup_partial(state: &AppState, video: &Video) {
    let Ok(Some(playlist)) = db::get_playlist(&state.db(), &video.playlist_id) else {
        return;
    };
    let workdir = state.temp_dir().join(&playlist.folder);
    let prefix = episode_prefix(&playlist.folder, video.episode);
    let Ok(mut entries) = tokio::fs::read_dir(&workdir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) || name.starts_with(&video.yt_id) {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
}

async fn process(
    app: &AppHandle,
    state: &Arc<AppState>,
    video: &Video,
    cancel: &CancellationToken,
) -> Result<String> {
    let settings = state.settings();
    if settings.library_dir.trim().is_empty() {
        bail!("Choisis le dossier de la bibliothèque dans les paramètres.");
    }
    let Some(playlist) = db::get_playlist(&state.db(), &video.playlist_id)? else {
        bail!("Playlist introuvable.");
    };
    let tools = &state.tools;

    emit_progress(app, video.id, 0.0, "Analyse");
    let workdir = state.temp_dir().join(&playlist.folder);
    tokio::fs::create_dir_all(&workdir).await?;
    let info = ytdlp::fetch_video(tools, &video.yt_id, cancel).await?;
    let info_path = workdir.join(format!("{}.info.json", video.yt_id));
    tokio::fs::write(&info_path, serde_json::to_vec(&info)?).await?;

    let title = info
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or(&video.title)
        .to_string();
    let max_height = playlist.max_height.unwrap_or(settings.max_height);
    let selection = ytdlp::select_formats(&info, max_height, settings.default_audio != "vo");
    let base = format!(
        "{}{}",
        episode_prefix(&playlist.folder, video.episode),
        sanitize(&title, 100)
    );

    let job = DownloadJob {
        video_id: video.id,
        info_json: &info_path,
        selection: &selection,
        workdir: &workdir,
        base: &base,
        auto_subs: settings.auto_subs,
        rate_limit: &settings.rate_limit,
    };
    let file = ytdlp::download(app, tools, &job, cancel).await?;
    let _ = tokio::fs::remove_file(&info_path).await;
    // yt-dlp leaves the converted .srt next to the MKV once embedded.
    if let Ok(mut entries) = tokio::fs::read_dir(&workdir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&base) && (name.ends_with(".srt") || name.ends_with(".vtt")) {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }

    emit_progress(app, video.id, 97.0, "Assemblage");
    if let Err(e) = ytdlp::tag_tracks(tools, &file, &selection).await {
        // Track names are cosmetic, keep the file.
        eprintln!("tag tracks: {e:#}");
    }
    let channel = info
        .get("channel")
        .and_then(|c| c.as_str())
        .or(video.channel.as_deref());
    let nfo = workdir.join(format!("{base}.nfo"));
    tokio::fs::write(&nfo, library::episode_nfo(&info, &title, video.episode, channel)).await?;
    let thumb = workdir.join(format!("{base}-thumb.jpg"));

    if cancel.is_cancelled() {
        return Err(Stopped.into());
    }
    emit_progress(app, video.id, 98.0, "Copie vers la bibliothèque");
    let season = library::ensure_series(&settings.library_dir, &playlist).await?;
    let dest = season.join(format!("{base}.mkv"));
    library::move_verified(&file, &dest).await?;
    library::move_verified(&nfo, &season.join(format!("{base}.nfo"))).await?;
    if thumb.exists() {
        library::move_verified(&thumb, &season.join(format!("{base}-thumb.jpg"))).await?;
    }
    Ok(dest.display().to_string())
}
