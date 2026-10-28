use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::db::{self, Playlist, Video};
use crate::library::{self, JellyfinInfo};
use crate::queue::notify_changed;
use crate::settings::Settings;
use crate::sources;
use crate::state::{AppState, StopReason, ToolsInstall};
use crate::tools::ToolsVersions;
use crate::ytdlp::{self, RemoteInfo};

type AppStateRef<'a> = State<'a, Arc<AppState>>;
type CmdResult<T> = Result<T, String>;

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[tauri::command]
pub fn get_settings(state: AppStateRef) -> Settings {
    state.settings()
}

#[tauri::command]
pub fn save_settings(state: AppStateRef, settings: Settings) -> CmdResult<()> {
    settings.save(&state.paths.settings_file).map_err(err)?;
    *state.settings.write().unwrap_or_else(|e| e.into_inner()) = settings;
    state.queue.notify.notify_one();
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsStatus {
    ready: bool,
    install: ToolsInstall,
    default_temp: String,
}

#[tauri::command]
pub fn tools_status(state: AppStateRef) -> ToolsStatus {
    ToolsStatus {
        ready: state.tools.ready(),
        install: state.tools_install.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        default_temp: state.paths.default_temp.display().to_string(),
    }
}

/// Downloads missing tools. Also called once at startup.
pub async fn install_tools_inner(app: &AppHandle, state: &AppState) {
    {
        let mut install = state.tools_install.lock().unwrap_or_else(|e| e.into_inner());
        if install.installing {
            return;
        }
        *install = ToolsInstall { installing: true, error: None };
    }
    let _ = app.emit("tools-changed", ());
    let result = state.tools.ensure(app).await;
    *state.tools_install.lock().unwrap_or_else(|e| e.into_inner()) = ToolsInstall {
        installing: false,
        error: result.err().map(err),
    };
    let _ = app.emit("tools-changed", ());
    state.queue.notify.notify_one();
}

#[tauri::command]
pub async fn install_tools(app: AppHandle, state: AppStateRef<'_>) -> CmdResult<()> {
    install_tools_inner(&app, &state).await;
    Ok(())
}

#[tauri::command]
pub async fn tools_versions(state: AppStateRef<'_>) -> CmdResult<ToolsVersions> {
    Ok(state.tools.versions().await)
}

#[tauri::command]
pub async fn update_ytdlp(state: AppStateRef<'_>) -> CmdResult<String> {
    state.tools.update_ytdlp().await.map_err(err)
}

#[tauri::command]
pub async fn fetch_url(state: AppStateRef<'_>, url: String) -> CmdResult<RemoteInfo> {
    if !state.tools.ready() {
        return Err("Les outils sont encore en cours d'installation.".into());
    }
    ytdlp::fetch_remote(&state.tools, url.trim()).await.map_err(err)
}

#[tauri::command]
pub fn add_source(
    app: AppHandle,
    state: AppStateRef,
    info: RemoteInfo,
    auto_sync: bool,
    max_height: Option<u32>,
    download: bool,
) -> CmdResult<String> {
    sources::add(&app, &state, &info, auto_sync, max_height, download).map_err(err)
}

#[tauri::command]
pub fn list_playlists(state: AppStateRef) -> CmdResult<Vec<Playlist>> {
    db::list_playlists(&state.db()).map_err(err)
}

#[derive(Serialize)]
pub struct PlaylistDetail {
    playlist: Playlist,
    videos: Vec<Video>,
}

#[tauri::command]
pub fn get_playlist(state: AppStateRef, id: String) -> CmdResult<Option<PlaylistDetail>> {
    let conn = state.db();
    let Some(playlist) = db::get_playlist(&conn, &id).map_err(err)? else {
        return Ok(None);
    };
    let videos = db::list_videos(&conn, &id).map_err(err)?;
    Ok(Some(PlaylistDetail { playlist, videos }))
}

#[tauri::command]
pub fn update_playlist(
    app: AppHandle,
    state: AppStateRef,
    id: String,
    auto_sync: bool,
    max_height: Option<u32>,
) -> CmdResult<()> {
    db::update_playlist_options(&state.db(), &id, auto_sync, max_height).map_err(err)?;
    notify_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn delete_playlist(app: AppHandle, state: AppStateRef, id: String) -> CmdResult<()> {
    let videos = db::list_videos(&state.db(), &id).map_err(err)?;
    for v in videos.iter().filter(|v| v.status == "downloading") {
        state.stop(v.id, StopReason::Remove);
    }
    db::delete_playlist(&state.db(), &id).map_err(err)?;
    notify_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn sync_playlist(app: AppHandle, state: AppStateRef<'_>, id: String) -> CmdResult<usize> {
    sources::sync(&app, &state, &id).await.map_err(err)
}

#[tauri::command]
pub fn download_playlist(app: AppHandle, state: AppStateRef, id: String) -> CmdResult<usize> {
    let n = db::queue_playlist(&state.db(), &id).map_err(err)?;
    notify_changed(&app);
    state.queue.notify.notify_one();
    Ok(n)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueInfo {
    paused: bool,
    videos: Vec<Video>,
}

#[tauri::command]
pub fn list_queue(state: AppStateRef) -> CmdResult<QueueInfo> {
    Ok(QueueInfo {
        paused: state.queue.paused.load(Ordering::SeqCst),
        videos: db::queue_list(&state.db()).map_err(err)?,
    })
}

#[tauri::command]
pub fn queue_videos(app: AppHandle, state: AppStateRef, ids: Vec<i64>) -> CmdResult<usize> {
    let n = db::queue_videos(&state.db(), &ids).map_err(err)?;
    notify_changed(&app);
    state.queue.notify.notify_one();
    Ok(n)
}

#[tauri::command]
pub fn pause_video(app: AppHandle, state: AppStateRef, id: i64) -> CmdResult<()> {
    if !state.stop(id, StopReason::Pause) {
        let conn = state.db();
        if let Some(v) = db::get_video(&conn, id).map_err(err)? {
            if v.status == "queued" {
                db::set_status(&conn, id, "paused", None).map_err(err)?;
            }
        }
    }
    notify_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn remove_from_queue(app: AppHandle, state: AppStateRef, id: i64) -> CmdResult<()> {
    if !state.stop(id, StopReason::Remove) {
        db::set_status(&state.db(), id, "new", None).map_err(err)?;
    }
    notify_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn set_queue_paused(app: AppHandle, state: AppStateRef, paused: bool) {
    state.queue.paused.store(paused, Ordering::SeqCst);
    if paused {
        state.stop_all(StopReason::Requeue);
    }
    state.queue.notify.notify_one();
    notify_changed(&app);
}

#[tauri::command]
pub fn open_playlist_folder(state: AppStateRef, id: String) -> CmdResult<()> {
    let Some(p) = db::get_playlist(&state.db(), &id).map_err(err)? else {
        return Err("Playlist introuvable.".into());
    };
    let dir = library::series_dir(&state.settings().library_dir, &p);
    if !dir.exists() {
        return Err("Le dossier n'existe pas encore (aucune vidéo copiée).".into());
    }
    tauri_plugin_opener::open_path(dir, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_jellyfin(url: String, api_key: String) -> CmdResult<JellyfinInfo> {
    library::jellyfin_test(&url, &api_key).await.map_err(err)
}

#[tauri::command]
pub async fn refresh_jellyfin(state: AppStateRef<'_>) -> CmdResult<()> {
    let s = state.settings();
    library::jellyfin_refresh(&s.jellyfin_url, &s.jellyfin_api_key)
        .await
        .map_err(err)
}
