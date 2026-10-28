use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

use crate::util::now;

/// Virtual playlist that holds videos added one by one.
pub const SINGLES_ID: &str = "__singles__";

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
CREATE TABLE IF NOT EXISTS playlists (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    channel     TEXT,
    url         TEXT NOT NULL,
    thumbnail   TEXT,
    description TEXT,
    auto_sync   INTEGER NOT NULL DEFAULT 0,
    max_height  INTEGER,
    folder      TEXT NOT NULL,
    last_sync   INTEGER,
    created_at  INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS videos (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    yt_id       TEXT NOT NULL,
    playlist_id TEXT NOT NULL,
    episode     INTEGER NOT NULL,
    title       TEXT NOT NULL,
    channel     TEXT,
    thumbnail   TEXT,
    duration    REAL,
    status      TEXT NOT NULL,
    error       TEXT,
    file_path   TEXT,
    queued_at   INTEGER,
    updated_at  INTEGER,
    UNIQUE (yt_id, playlist_id)
);
CREATE INDEX IF NOT EXISTS videos_status ON videos (status);
";

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub title: String,
    pub channel: Option<String>,
    pub url: String,
    pub thumbnail: Option<String>,
    pub description: Option<String>,
    pub auto_sync: bool,
    pub max_height: Option<u32>,
    pub folder: String,
    pub last_sync: Option<i64>,
    pub created_at: i64,
    pub total: i64,
    pub done: i64,
    pub pending: i64,
    pub errors: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub id: i64,
    pub yt_id: String,
    pub playlist_id: String,
    pub episode: i64,
    pub title: String,
    pub channel: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<f64>,
    pub status: String,
    pub error: Option<String>,
    pub file_path: Option<String>,
    pub queued_at: Option<i64>,
    pub playlist_title: Option<String>,
}

/// New playlist row, before it is stored.
pub struct NewPlaylist<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub channel: Option<&'a str>,
    pub url: &'a str,
    pub thumbnail: Option<&'a str>,
    pub description: Option<&'a str>,
    pub auto_sync: bool,
    pub max_height: Option<u32>,
    pub folder: &'a str,
}

/// New video row, before it is stored.
pub struct NewVideo<'a> {
    pub yt_id: &'a str,
    pub title: &'a str,
    pub channel: Option<&'a str>,
    pub thumbnail: Option<&'a str>,
    pub duration: Option<f64>,
}

const PLAYLIST_SELECT: &str = "
SELECT p.id, p.title, p.channel, p.url, p.thumbnail, p.description, p.auto_sync, p.max_height,
       p.folder, p.last_sync, p.created_at,
       (SELECT COUNT(*) FROM videos v WHERE v.playlist_id = p.id),
       (SELECT COUNT(*) FROM videos v WHERE v.playlist_id = p.id AND v.status = 'done'),
       (SELECT COUNT(*) FROM videos v WHERE v.playlist_id = p.id
            AND v.status IN ('queued', 'downloading', 'paused')),
       (SELECT COUNT(*) FROM videos v WHERE v.playlist_id = p.id AND v.status = 'error')
FROM playlists p";

fn playlist_from_row(r: &Row) -> rusqlite::Result<Playlist> {
    Ok(Playlist {
        id: r.get(0)?,
        title: r.get(1)?,
        channel: r.get(2)?,
        url: r.get(3)?,
        thumbnail: r.get(4)?,
        description: r.get(5)?,
        auto_sync: r.get::<_, i64>(6)? != 0,
        max_height: r.get(7)?,
        folder: r.get(8)?,
        last_sync: r.get(9)?,
        created_at: r.get(10)?,
        total: r.get(11)?,
        done: r.get(12)?,
        pending: r.get(13)?,
        errors: r.get(14)?,
    })
}

const VIDEO_SELECT: &str = "
SELECT v.id, v.yt_id, v.playlist_id, v.episode, v.title, v.channel, v.thumbnail, v.duration,
       v.status, v.error, v.file_path, v.queued_at, p.title
FROM videos v LEFT JOIN playlists p ON p.id = v.playlist_id";

fn video_from_row(r: &Row) -> rusqlite::Result<Video> {
    Ok(Video {
        id: r.get(0)?,
        yt_id: r.get(1)?,
        playlist_id: r.get(2)?,
        episode: r.get(3)?,
        title: r.get(4)?,
        channel: r.get(5)?,
        thumbnail: r.get(6)?,
        duration: r.get(7)?,
        status: r.get(8)?,
        error: r.get(9)?,
        file_path: r.get(10)?,
        queued_at: r.get(11)?,
        playlist_title: r.get(12)?,
    })
}

pub fn list_playlists(conn: &Connection) -> Result<Vec<Playlist>> {
    let sql = format!(
        "{PLAYLIST_SELECT} ORDER BY CASE WHEN p.id = '{SINGLES_ID}' THEN 1 ELSE 0 END, p.created_at DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], playlist_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get_playlist(conn: &Connection, id: &str) -> Result<Option<Playlist>> {
    let sql = format!("{PLAYLIST_SELECT} WHERE p.id = ?1");
    Ok(conn.query_row(&sql, [id], playlist_from_row).optional()?)
}

pub fn insert_playlist(conn: &Connection, p: &NewPlaylist) -> Result<()> {
    conn.execute(
        "INSERT INTO playlists (id, title, channel, url, thumbnail, description, auto_sync,
                                max_height, folder, last_sync, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            p.id,
            p.title,
            p.channel,
            p.url,
            p.thumbnail,
            p.description,
            p.auto_sync as i64,
            p.max_height,
            p.folder,
            now()
        ],
    )?;
    Ok(())
}

pub fn update_playlist_options(
    conn: &Connection,
    id: &str,
    auto_sync: bool,
    max_height: Option<u32>,
) -> Result<()> {
    conn.execute(
        "UPDATE playlists SET auto_sync = ?2, max_height = ?3 WHERE id = ?1",
        params![id, auto_sync as i64, max_height],
    )?;
    Ok(())
}

pub fn update_playlist_meta(
    conn: &Connection,
    id: &str,
    thumbnail: Option<&str>,
    description: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE playlists SET thumbnail = COALESCE(?2, thumbnail),
                              description = COALESCE(?3, description),
                              last_sync = ?4
         WHERE id = ?1",
        params![id, thumbnail, description, now()],
    )?;
    Ok(())
}

pub fn delete_playlist(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM videos WHERE playlist_id = ?1", [id])?;
    conn.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
    Ok(())
}

/// Folder names must stay unique, two playlists can share a title.
pub fn unique_folder(conn: &Connection, base: &str) -> Result<String> {
    let mut candidate = base.to_string();
    let mut n = 2;
    loop {
        let taken: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM playlists WHERE lower(folder) = lower(?1))",
            [&candidate],
            |r| r.get(0),
        )?;
        if !taken {
            return Ok(candidate);
        }
        candidate = format!("{base} ({n})");
        n += 1;
    }
}

pub fn list_videos(conn: &Connection, playlist_id: &str) -> Result<Vec<Video>> {
    let sql = format!("{VIDEO_SELECT} WHERE v.playlist_id = ?1 ORDER BY v.episode");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([playlist_id], video_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get_video(conn: &Connection, id: i64) -> Result<Option<Video>> {
    let sql = format!("{VIDEO_SELECT} WHERE v.id = ?1");
    Ok(conn.query_row(&sql, [id], video_from_row).optional()?)
}

pub fn known_ids(conn: &Connection, playlist_id: &str) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT yt_id FROM videos WHERE playlist_id = ?1")?;
    let rows = stmt.query_map([playlist_id], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Appends a video at the end of the playlist. Returns false if it was already there.
pub fn insert_video(conn: &Connection, playlist_id: &str, v: &NewVideo, queue: bool) -> Result<bool> {
    let episode: i64 = conn.query_row(
        "SELECT COALESCE(MAX(episode), 0) + 1 FROM videos WHERE playlist_id = ?1",
        [playlist_id],
        |r| r.get(0),
    )?;
    let ts = now();
    let changed = conn.execute(
        "INSERT OR IGNORE INTO videos (yt_id, playlist_id, episode, title, channel, thumbnail,
                                       duration, status, queued_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            v.yt_id,
            playlist_id,
            episode,
            v.title,
            v.channel,
            v.thumbnail,
            v.duration,
            if queue { "queued" } else { "new" },
            if queue { Some(ts) } else { None },
            ts
        ],
    )?;
    Ok(changed > 0)
}

pub fn set_status(conn: &Connection, id: i64, status: &str, error: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE videos SET status = ?2, error = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, status, error, now()],
    )?;
    Ok(())
}

pub fn set_done(conn: &Connection, id: i64, file_path: &str) -> Result<()> {
    conn.execute(
        "UPDATE videos SET status = 'done', error = NULL, file_path = ?2, updated_at = ?3
         WHERE id = ?1",
        params![id, file_path, now()],
    )?;
    Ok(())
}

/// Puts videos back in the queue (only those not already done or running).
pub fn queue_videos(conn: &Connection, ids: &[i64]) -> Result<usize> {
    let ts = now();
    let mut n = 0;
    for id in ids {
        n += conn.execute(
            "UPDATE videos SET status = 'queued', error = NULL, queued_at = ?2, updated_at = ?2
             WHERE id = ?1 AND status IN ('new', 'error', 'paused')",
            params![id, ts],
        )?;
    }
    Ok(n)
}

pub fn queue_playlist(conn: &Connection, playlist_id: &str) -> Result<usize> {
    let ts = now();
    Ok(conn.execute(
        "UPDATE videos SET status = 'queued', error = NULL, queued_at = ?2, updated_at = ?2
         WHERE playlist_id = ?1 AND status IN ('new', 'error', 'paused')",
        params![playlist_id, ts],
    )?)
}

pub fn next_queued(conn: &Connection) -> Result<Option<Video>> {
    let sql = format!(
        "{VIDEO_SELECT} WHERE v.status = 'queued' ORDER BY v.queued_at, v.episode, v.id LIMIT 1"
    );
    Ok(conn.query_row(&sql, [], video_from_row).optional()?)
}

pub fn queue_list(conn: &Connection) -> Result<Vec<Video>> {
    let sql = format!(
        "{VIDEO_SELECT} WHERE v.status IN ('downloading', 'queued', 'paused', 'error')
         ORDER BY CASE v.status WHEN 'downloading' THEN 0 WHEN 'queued' THEN 1
                                WHEN 'paused' THEN 2 ELSE 3 END,
                  v.queued_at, v.episode, v.id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], video_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn has_pending(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM videos WHERE status IN ('queued', 'downloading'))",
        [],
        |r| r.get(0),
    )?)
}

/// After a crash or a restart nothing is running anymore.
pub fn reset_downloading(conn: &Connection) -> Result<()> {
    conn.execute(
        "UPDATE videos SET status = 'queued' WHERE status = 'downloading'",
        [],
    )?;
    Ok(())
}
