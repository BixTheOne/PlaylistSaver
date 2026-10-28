import {
  ArrowLeft,
  Download,
  FolderOpen,
  Pause,
  Play,
  RefreshCw,
  RotateCcw,
  Trash2,
  X,
} from "lucide-react";
import { ask } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { api, errorText, Playlist, SINGLES_ID, Video } from "../lib/api";
import { plural, QUALITY_OPTIONS, relative } from "../lib/format";
import { useStore } from "../lib/store";
import {
  Button,
  EmptyState,
  IconButton,
  ProgressBar,
  Select,
  Spinner,
  StatusLabel,
  Thumb,
  Toggle,
} from "../components/ui";

const QUALITY_WITH_DEFAULT = [{ value: -1, label: "Qualité par défaut" }, ...QUALITY_OPTIONS];

export function VideoActions({ v }: { v: Video }) {
  const { toast } = useStore();
  const run = (p: Promise<unknown>) => p.catch((e) => toast(errorText(e), "error"));
  switch (v.status) {
    case "new":
      return (
        <IconButton label="Télécharger" onClick={() => run(api.queueVideos([v.id]))}>
          <Download size={18} />
        </IconButton>
      );
    case "error":
      return (
        <IconButton label="Réessayer" onClick={() => run(api.queueVideos([v.id]))}>
          <RotateCcw size={18} />
        </IconButton>
      );
    case "paused":
      return (
        <>
          <IconButton label="Reprendre" onClick={() => run(api.queueVideos([v.id]))}>
            <Play size={18} />
          </IconButton>
          <IconButton label="Retirer de la file" onClick={() => run(api.removeFromQueue(v.id))}>
            <X size={18} />
          </IconButton>
        </>
      );
    case "queued":
    case "downloading":
      return (
        <>
          <IconButton label="Pause" onClick={() => run(api.pauseVideo(v.id))}>
            <Pause size={18} />
          </IconButton>
          <IconButton label="Retirer de la file" onClick={() => run(api.removeFromQueue(v.id))}>
            <X size={18} />
          </IconButton>
        </>
      );
    default:
      return null;
  }
}

function VideoRow({ v, index }: { v: Video; index: number }) {
  const { progress } = useStore();
  const p = progress[v.id];
  return (
    <li className="group flex items-center gap-3 rounded-xl px-2 py-2 hover:bg-surface">
      <span className="w-7 shrink-0 text-center text-sm text-muted">{index}</span>
      <Thumb src={v.thumbnail} seconds={v.duration} className="w-40 rounded-lg" />
      <div className="min-w-0 flex-1">
        <p className="line-clamp-2 text-sm font-medium">{v.title}</p>
        {v.channel && <p className="mt-0.5 truncate text-xs text-muted">{v.channel}</p>}
        <div className="mt-1 flex items-center gap-2">
          <StatusLabel status={v.status} />
          {v.status === "downloading" && p && (
            <span className="text-xs text-muted">
              {p.stage} · {Math.round(p.percent)} %
            </span>
          )}
        </div>
        {v.status === "downloading" && <ProgressBar value={p?.percent ?? 0} className="mt-1.5 max-w-sm" />}
        {v.status === "error" && v.error && (
          <p className="mt-1 line-clamp-2 text-xs text-danger" title={v.error}>
            {v.error}
          </p>
        )}
      </div>
      <div className="flex shrink-0 items-center">
        <VideoActions v={v} />
      </div>
    </li>
  );
}

export function PlaylistView({ id }: { id: string }) {
  const { tick, go, toast } = useStore();
  const [data, setData] = useState<{ playlist: Playlist; videos: Video[] } | null | undefined>();
  const [syncing, setSyncing] = useState(false);

  useEffect(() => {
    api.getPlaylist(id).then(setData).catch(() => setData(null));
  }, [id, tick]);

  if (data === undefined) {
    return (
      <div className="flex justify-center py-24">
        <Spinner />
      </div>
    );
  }
  if (data === null) {
    return <EmptyState icon={<X size={48} />} title="Playlist introuvable" />;
  }

  const { playlist: p, videos } = data;
  const isSingles = p.id === SINGLES_ID;
  const missing = videos.filter((v) => v.status === "new" || v.status === "error" || v.status === "paused");

  const sync = async () => {
    setSyncing(true);
    try {
      const n = await api.syncPlaylist(p.id);
      toast(n ? `${plural(n, "nouvelle vidéo")} ajoutée${n > 1 ? "s" : ""} à la file` : "Playlist déjà à jour");
    } catch (e) {
      toast(errorText(e), "error");
    } finally {
      setSyncing(false);
    }
  };

  const remove = async () => {
    const ok = await ask(`Retirer « ${p.title} » de l'app ? Les fichiers déjà téléchargés restent sur le disque.`, {
      title: "Retirer la playlist",
      kind: "warning",
      okLabel: "Retirer",
      cancelLabel: "Annuler",
    });
    if (!ok) return;
    try {
      await api.deletePlaylist(p.id);
      go({ name: "library" });
    } catch (e) {
      toast(errorText(e), "error");
    }
  };

  const setOptions = (autoSync: boolean, maxHeight: number | null) =>
    api.updatePlaylist(p.id, autoSync, maxHeight).catch((e) => toast(errorText(e), "error"));

  return (
    <div className="px-6 pt-2 pb-10">
      <button
        onClick={() => go({ name: "library" })}
        className="mb-3 -ml-2 flex items-center gap-1 rounded-full px-2 py-1 text-sm text-muted hover:bg-surface hover:text-text"
      >
        <ArrowLeft size={16} /> Bibliothèque
      </button>

      <div className="flex gap-6">
        <Thumb src={p.thumbnail} className="w-80 rounded-xl" />
        <div className="min-w-0 flex-1">
          <h1 className="text-2xl leading-tight font-bold">{p.title}</h1>
          <p className="mt-2 text-sm text-muted">
            {[p.channel, plural(p.total, "vidéo"), `${p.done} téléchargée${p.done > 1 ? "s" : ""}`]
              .filter(Boolean)
              .join(" · ")}
          </p>
          {!isSingles && (
            <p className="mt-0.5 text-xs text-muted">Dernière synchro : {relative(p.lastSync)}</p>
          )}

          <div className="mt-4 flex flex-wrap items-center gap-2">
            <Button
              variant="primary"
              icon={<Download size={18} />}
              disabled={!missing.length}
              onClick={() => api.downloadPlaylist(p.id).catch((e) => toast(errorText(e), "error"))}
            >
              {missing.length ? `Télécharger (${missing.length})` : "Tout est téléchargé"}
            </Button>
            {!isSingles && (
              <Button icon={<RefreshCw size={18} />} loading={syncing} onClick={sync}>
                Synchroniser
              </Button>
            )}
            <Button
              icon={<FolderOpen size={18} />}
              onClick={() => api.openPlaylistFolder(p.id).catch((e) => toast(errorText(e), "error"))}
            >
              Ouvrir le dossier
            </Button>
            <IconButton label="Retirer de l'app" onClick={remove}>
              <Trash2 size={18} />
            </IconButton>
          </div>

          <div className="mt-4 flex flex-wrap items-center gap-5 text-sm">
            {!isSingles && (
              <label className="flex cursor-pointer items-center gap-2">
                <Toggle checked={p.autoSync} onChange={(v) => setOptions(v, p.maxHeight)} label="Synchro auto" />
                Synchro auto
              </label>
            )}
            <Select
              value={p.maxHeight ?? -1}
              options={QUALITY_WITH_DEFAULT}
              onChange={(v) => setOptions(p.autoSync, v < 0 ? null : v)}
            />
          </div>
        </div>
      </div>

      <ol className="mt-8 flex flex-col">
        {videos.map((v) => (
          <VideoRow key={v.id} v={v} index={v.episode} />
        ))}
      </ol>
    </div>
  );
}
