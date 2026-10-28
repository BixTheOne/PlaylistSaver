use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::settings::Settings;
use crate::tools::Tools;

pub struct Paths {
    pub settings_file: PathBuf,
    pub default_temp: PathBuf,
}

/// Why a running job was interrupted, decides the status it falls back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// Pause on this video only.
    Pause,
    /// Global pause or app exit: go back to the queue.
    Requeue,
    /// Removed from the queue.
    Remove,
}

pub struct Running {
    pub token: CancellationToken,
    pub reason: Arc<Mutex<StopReason>>,
}

#[derive(Default)]
pub struct QueueState {
    pub paused: AtomicBool,
    pub running: Mutex<HashMap<i64, Running>>,
    pub notify: Notify,
}

#[derive(Default, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsInstall {
    pub installing: bool,
    pub error: Option<String>,
}

pub struct AppState {
    pub paths: Paths,
    pub tools: Tools,
    pub db: Mutex<Connection>,
    pub settings: RwLock<Settings>,
    pub queue: QueueState,
    pub tools_install: Mutex<ToolsInstall>,
}

impl AppState {
    pub fn db(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn temp_dir(&self) -> PathBuf {
        let s = self.settings();
        if s.temp_dir.trim().is_empty() {
            self.paths.default_temp.clone()
        } else {
            PathBuf::from(s.temp_dir.trim())
        }
    }

    pub fn stop(&self, video_id: i64, reason: StopReason) -> bool {
        let running = self.queue.running.lock().unwrap_or_else(|e| e.into_inner());
        match running.get(&video_id) {
            Some(job) => {
                *job.reason.lock().unwrap_or_else(|e| e.into_inner()) = reason;
                job.token.cancel();
                true
            }
            None => false,
        }
    }

    pub fn stop_all(&self, reason: StopReason) {
        let ids: Vec<i64> = self
            .queue
            .running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .copied()
            .collect();
        for id in ids {
            self.stop(id, reason);
        }
    }
}
