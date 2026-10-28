import { invoke } from "@tauri-apps/api/core";

export type Theme = "light" | "dark" | "system";

export interface Settings {
  libraryDir: string;
  tempDir: string;
  maxHeight: number;
  autoSubs: boolean;
  defaultAudio: "vf" | "vo";
  rateLimit: string;
  concurrency: number;
  syncIntervalHours: number;
  singlesTitle: string;
  jellyfinUrl: string;
  jellyfinApiKey: string;
  theme: Theme;
  closeToTray: boolean;
}

export interface RemoteEntry {
  id: string;
  title: string;
  channel: string | null;
  thumbnail: string | null;
  duration: number | null;
}

export interface RemoteInfo {
  kind: "playlist" | "video";
  id: string;
  title: string;
  channel: string | null;
  url: string;
  thumbnail: string | null;
  description: string | null;
  entries: RemoteEntry[];
}

export interface Playlist {
  id: string;
  title: string;
  channel: string | null;
  url: string;
  thumbnail: string | null;
  description: string | null;
  autoSync: boolean;
  maxHeight: number | null;
  folder: string;
  lastSync: number | null;
  createdAt: number;
  total: number;
  done: number;
  pending: number;
  errors: number;
}

export type VideoStatus =
  | "new"
  | "queued"
  | "downloading"
  | "paused"
  | "done"
  | "error";

export interface Video {
  id: number;
  ytId: string;
  playlistId: string;
  episode: number;
  title: string;
  channel: string | null;
  thumbnail: string | null;
  duration: number | null;
  status: VideoStatus;
  error: string | null;
  filePath: string | null;
  queuedAt: number | null;
  playlistTitle: string | null;
}

export interface Progress {
  id: number;
  percent: number;
  speed: number | null;
  eta: number | null;
  stage: string;
}

export interface ToolsStatus {
  ready: boolean;
  install: { installing: boolean; error: string | null };
  defaultTemp: string;
}

export interface ToolsVersions {
  ytdlp: string | null;
  ffmpeg: string | null;
  deno: string | null;
}

export const SINGLES_ID = "__singles__";

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  toolsStatus: () => invoke<ToolsStatus>("tools_status"),
  installTools: () => invoke<void>("install_tools"),
  toolsVersions: () => invoke<ToolsVersions>("tools_versions"),
  updateYtdlp: () => invoke<string>("update_ytdlp"),
  fetchUrl: (url: string) => invoke<RemoteInfo>("fetch_url", { url }),
  addSource: (info: RemoteInfo, autoSync: boolean, maxHeight: number | null, download: boolean) =>
    invoke<string>("add_source", { info, autoSync, maxHeight, download }),
  listPlaylists: () => invoke<Playlist[]>("list_playlists"),
  getPlaylist: (id: string) =>
    invoke<{ playlist: Playlist; videos: Video[] } | null>("get_playlist", { id }),
  updatePlaylist: (id: string, autoSync: boolean, maxHeight: number | null) =>
    invoke<void>("update_playlist", { id, autoSync, maxHeight }),
  deletePlaylist: (id: string) => invoke<void>("delete_playlist", { id }),
  syncPlaylist: (id: string) => invoke<number>("sync_playlist", { id }),
  downloadPlaylist: (id: string) => invoke<number>("download_playlist", { id }),
  listQueue: () => invoke<{ paused: boolean; videos: Video[] }>("list_queue"),
  queueVideos: (ids: number[]) => invoke<number>("queue_videos", { ids }),
  pauseVideo: (id: number) => invoke<void>("pause_video", { id }),
  removeFromQueue: (id: number) => invoke<void>("remove_from_queue", { id }),
  setQueuePaused: (paused: boolean) => invoke<void>("set_queue_paused", { paused }),
  openPlaylistFolder: (id: string) => invoke<void>("open_playlist_folder", { id }),
  testJellyfin: (url: string, apiKey: string) =>
    invoke<{ serverName: string; version: string }>("test_jellyfin", { url, apiKey }),
  refreshJellyfin: () => invoke<void>("refresh_jellyfin"),
};

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}
