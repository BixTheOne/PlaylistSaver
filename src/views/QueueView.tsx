import { CheckCircle2, Pause, Play, RotateCcw } from "lucide-react";
import { ReactNode, useEffect, useState } from "react";
import { api, errorText, Video } from "../lib/api";
import { eta, speed } from "../lib/format";
import { useStore } from "../lib/store";
import { Button, EmptyState, ProgressBar, Spinner, Thumb } from "../components/ui";
import { VideoActions } from "./PlaylistView";

function Section({ title, count, children }: { title: string; count: number; children: ReactNode }) {
  if (!count) return null;
  return (
    <section className="mt-6">
      <h2 className="mb-2 text-base font-medium">
        {title} <span className="text-muted">· {count}</span>
      </h2>
      <ul className="flex flex-col">{children}</ul>
    </section>
  );
}

function QueueRow({ v }: { v: Video }) {
  const { progress, go } = useStore();
  const p = progress[v.id];
  const active = v.status === "downloading";
  return (
    <li className="flex items-center gap-4 rounded-xl px-2 py-2 hover:bg-surface">
      <Thumb src={v.thumbnail} seconds={v.duration} className={`${active ? "w-48" : "w-32"} rounded-lg`} />
      <div className="min-w-0 flex-1">
        <p className="line-clamp-2 text-sm font-medium">{v.title}</p>
        <button
          onClick={() => go({ name: "playlist", id: v.playlistId })}
          className="mt-0.5 truncate text-xs text-muted hover:text-text"
        >
          {v.playlistTitle} · Épisode {v.episode}
        </button>
        {active && (
          <>
            <ProgressBar value={p?.percent ?? 0} className="mt-2" />
            <p className="mt-1.5 text-xs text-muted">
              {[
                p?.stage ?? "Démarrage",
                p ? `${Math.round(p.percent)} %` : null,
                speed(p?.speed),
                p?.eta ? `reste ${eta(p.eta)}` : null,
              ]
                .filter(Boolean)
                .join(" · ")}
            </p>
          </>
        )}
        {v.status === "error" && v.error && (
          <p className="mt-1 line-clamp-3 text-xs break-words text-danger" title={v.error}>
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

export function QueueView() {
  const { tick, toast } = useStore();
  const [data, setData] = useState<{ paused: boolean; videos: Video[] } | null>(null);

  useEffect(() => {
    api.listQueue().then(setData).catch(() => {});
  }, [tick]);

  if (!data) {
    return (
      <div className="flex justify-center py-24">
        <Spinner />
      </div>
    );
  }

  const by = (s: Video["status"]) => data.videos.filter((v) => v.status === s);
  const [running, queued, paused, errors] = [by("downloading"), by("queued"), by("paused"), by("error")];

  const togglePause = () =>
    api.setQueuePaused(!data.paused).catch((e) => toast(errorText(e), "error"));

  return (
    <div className="px-6 pt-4 pb-10">
      <div className="flex items-center justify-between gap-4">
        <h1 className="text-2xl font-bold">File d'attente</h1>
        <div className="flex gap-2">
          {errors.length > 0 && (
            <Button icon={<RotateCcw size={18} />} onClick={() => api.queueVideos(errors.map((v) => v.id))}>
              Tout réessayer
            </Button>
          )}
          {(running.length > 0 || queued.length > 0 || data.paused) && (
            <Button
              variant={data.paused ? "primary" : "secondary"}
              icon={data.paused ? <Play size={18} /> : <Pause size={18} />}
              onClick={togglePause}
            >
              {data.paused ? "Reprendre" : "Tout mettre en pause"}
            </Button>
          )}
        </div>
      </div>

      {data.paused && (
        <p className="mt-3 rounded-xl bg-surface px-4 py-3 text-sm text-muted">
          File en pause. Les téléchargements reprendront où ils se sont arrêtés.
        </p>
      )}

      {!data.videos.length && (
        <EmptyState
          icon={<CheckCircle2 size={56} strokeWidth={1.5} />}
          title="Rien à télécharger"
          text="Tout est à jour. Les nouvelles vidéos des playlists synchronisées apparaîtront ici."
        />
      )}

      <Section title="En cours" count={running.length}>
        {running.map((v) => (
          <QueueRow key={v.id} v={v} />
        ))}
      </Section>
      <Section title="En attente" count={queued.length}>
        {queued.map((v) => (
          <QueueRow key={v.id} v={v} />
        ))}
      </Section>
      <Section title="En pause" count={paused.length}>
        {paused.map((v) => (
          <QueueRow key={v.id} v={v} />
        ))}
      </Section>
      <Section title="Erreurs" count={errors.length}>
        {errors.map((v) => (
          <QueueRow key={v.id} v={v} />
        ))}
      </Section>
    </div>
  );
}
