//! Adding playlists / videos to the library and keeping playlists in sync.

use anyhow::{bail, Result};
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;

use crate::db::{self, NewPlaylist, NewVideo, SINGLES_ID};
use crate::queue::notify_changed;
use crate::state::AppState;
use crate::util::{now, sanitize};
use crate::ytdlp::{self, RemoteEntry, RemoteInfo};

fn new_video(e: &RemoteEntry) -> NewVideo<'_> {
    NewVideo {
        yt_id: &e.id,
        title: &e.title,
        channel: e.channel.as_deref(),
        thumbnail: e.thumbnail.as_deref(),
        duration: e.duration,
    }
}

/// Stores what the user previewed. Returns the playlist id it landed in.
pub fn add(
    app: &AppHandle,
    state: &AppState,
    info: &RemoteInfo,
    auto_sync: bool,
    max_height: Option<u32>,
    download: bool,
) -> Result<String> {
    let conn = state.db();
    let playlist_id = if info.kind == "video" {
        if db::get_playlist(&conn, SINGLES_ID)?.is_none() {
            let title = state.settings().singles_title;
            let folder = db::unique_folder(&conn, &sanitize(&title, 80))?;
            db::insert_playlist(
                &conn,
                &NewPlaylist {
                    id: SINGLES_ID,
                    title: &title,
                    channel: None,
                    url: "",
                    thumbnail: info.thumbnail.as_deref(),
                    description: None,
                    auto_sync: false,
                    max_height: None,
                    folder: &folder,
                },
            )?;
        }
        SINGLES_ID.to_string()
    } else {
        if info.id.is_empty() {
            bail!("Identifiant de playlist manquant.");
        }
        match db::get_playlist(&conn, &info.id)? {
            Some(_) => db::update_playlist_options(&conn, &info.id, auto_sync, max_height)?,
            None => {
                let folder = db::unique_folder(&conn, &sanitize(&info.title, 80))?;
                db::insert_playlist(
                    &conn,
                    &NewPlaylist {
                        id: &info.id,
                        title: &info.title,
                        channel: info.channel.as_deref(),
                        url: &info.url,
                        thumbnail: info.thumbnail.as_deref(),
                        description: info.description.as_deref(),
                        auto_sync,
                        max_height,
                        folder: &folder,
                    },
                )?;
            }
        }
        info.id.clone()
    };
    for entry in &info.entries {
        db::insert_video(&conn, &playlist_id, &new_video(entry), download)?;
    }
    if download {
        // Already known videos that were never downloaded go in the queue too.
        let known = db::list_videos(&conn, &playlist_id)?;
        let ids: Vec<i64> = known
            .iter()
            .filter(|v| info.entries.iter().any(|e| e.id == v.yt_id))
            .map(|v| v.id)
            .collect();
        db::queue_videos(&conn, &ids)?;
    }
    drop(conn);
    notify_changed(app);
    state.queue.notify.notify_one();
    Ok(playlist_id)
}

/// Fetches the playlist again and queues new videos. Returns how many were added.
pub async fn sync(app: &AppHandle, state: &AppState, playlist_id: &str) -> Result<usize> {
    if playlist_id == SINGLES_ID {
        return Ok(0);
    }
    let Some(playlist) = db::get_playlist(&state.db(), playlist_id)? else {
        bail!("Playlist introuvable.");
    };
    let info = ytdlp::fetch_remote(&state.tools, &playlist.url).await?;
    let added = {
        let conn = state.db();
        let known = db::known_ids(&conn, playlist_id)?;
        let mut added = 0;
        for entry in info.entries.iter().filter(|e| !known.contains(&e.id)) {
            if db::insert_video(&conn, playlist_id, &new_video(entry), true)? {
                added += 1;
            }
        }
        db::update_playlist_meta(
            &conn,
            playlist_id,
            info.thumbnail.as_deref(),
            info.description.as_deref(),
        )?;
        added
    };
    notify_changed(app);
    state.queue.notify.notify_one();
    Ok(added)
}

/// Background loop for playlists with automatic sync enabled.
pub fn start_scheduler(app: AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            let hours = state.settings().sync_interval_hours as i64;
            if hours > 0 && state.tools.ready() {
                let due: Vec<String> = db::list_playlists(&state.db())
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|p| p.auto_sync && p.id != SINGLES_ID)
                    .filter(|p| p.last_sync.is_none_or(|t| now() - t >= hours * 3600))
                    .map(|p| p.id)
                    .collect();
                for id in due {
                    if let Err(e) = sync(&app, &state, &id).await {
                        eprintln!("sync {id}: {e:#}");
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(600)).await;
        }
    });
}
