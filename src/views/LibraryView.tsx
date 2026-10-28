import { ListVideo, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { api, Playlist } from "../lib/api";
import { plural } from "../lib/format";
import { useStore } from "../lib/store";
import { EmptyState, ProgressBar, Spinner, Thumb } from "../components/ui";

function PlaylistCard({ p, onOpen }: { p: Playlist; onOpen: () => void }) {
  const ratio = p.total ? (p.done / p.total) * 100 : 0;
  const status = [
    `${p.done}/${p.total} téléchargées`,
    p.pending ? `${p.pending} en cours` : null,
    p.errors ? plural(p.errors, "erreur") : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <button onClick={onOpen} className="group text-left">
      <Thumb src={p.thumbnail} className="rounded-xl">
        <span className="absolute right-1.5 bottom-1.5 flex items-center gap-1 rounded bg-black/80 px-1.5 py-px text-xs font-medium text-white">
          <ListVideo size={14} />
          {plural(p.total, "vidéo")}
        </span>
        {p.autoSync && (
          <span
            className="absolute top-1.5 right-1.5 flex size-6 items-center justify-center rounded-full bg-black/70 text-white"
            title="Synchronisation automatique"
          >
            <RefreshCw size={13} />
          </span>
        )}
        <div className="absolute inset-0 bg-black/0 transition-colors group-hover:bg-black/10" />
      </Thumb>
      {ratio > 0 && ratio < 100 && <ProgressBar value={ratio} className="mt-2" />}
      <h3 className="mt-3 line-clamp-2 font-medium leading-snug">{p.title}</h3>
      {p.channel && <p className="mt-1 text-sm text-muted">{p.channel}</p>}
      <p className={`mt-0.5 text-sm ${p.errors ? "text-danger" : "text-muted"}`}>{status}</p>
    </button>
  );
}

export function LibraryView() {
  const { tick, go } = useStore();
  const [playlists, setPlaylists] = useState<Playlist[] | null>(null);

  useEffect(() => {
    api.listPlaylists().then(setPlaylists).catch(() => setPlaylists([]));
  }, [tick]);

  if (!playlists) {
    return (
      <div className="flex justify-center py-24">
        <Spinner />
      </div>
    );
  }

  if (!playlists.length) {
    return (
      <EmptyState
        icon={<ListVideo size={56} strokeWidth={1.5} />}
        title="Aucune playlist pour l'instant"
        text="Colle un lien de playlist ou de vidéo YouTube dans la barre en haut pour lancer ton premier téléchargement."
      />
    );
  }

  return (
    <div className="px-6 pt-4 pb-10">
      <h1 className="mb-5 text-2xl font-bold">Bibliothèque</h1>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-x-4 gap-y-8">
        {playlists.map((p) => (
          <PlaylistCard key={p.id} p={p} onOpen={() => go({ name: "playlist", id: p.id })} />
        ))}
      </div>
    </div>
  );
}
