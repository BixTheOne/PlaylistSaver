import { AlertCircle, X } from "lucide-react";
import { useEffect, useState } from "react";
import { api, errorText, RemoteInfo } from "../lib/api";
import { plural, QUALITY_OPTIONS } from "../lib/format";
import { useStore } from "../lib/store";
import { Button, IconButton, Select, Spinner, Thumb, Toggle } from "./ui";

const QUALITY_WITH_DEFAULT = [
  { value: -1, label: "Qualité par défaut" },
  ...QUALITY_OPTIONS,
];

export function AddDialog({ url, onClose }: { url: string; onClose: () => void }) {
  const { go, toast } = useStore();
  const [info, setInfo] = useState<RemoteInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [known, setKnown] = useState(false);
  const [autoSync, setAutoSync] = useState(true);
  const [quality, setQuality] = useState(-1);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let alive = true;
    setInfo(null);
    setError(null);
    api
      .fetchUrl(url)
      .then(async (i) => {
        if (!alive) return;
        setInfo(i);
        const lists = await api.listPlaylists();
        const existing = lists.find((p) => p.id === i.id);
        if (existing) {
          setKnown(true);
          setAutoSync(existing.autoSync);
          setQuality(existing.maxHeight ?? -1);
        }
      })
      .catch((e) => alive && setError(errorText(e)));
    return () => {
      alive = false;
    };
  }, [url]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const add = async (download: boolean) => {
    if (!info) return;
    setSaving(true);
    try {
      const id = await api.addSource(info, autoSync, quality < 0 ? null : quality, download);
      toast(download ? "Ajouté à la file d'attente" : "Ajouté à la bibliothèque");
      onClose();
      go({ name: "playlist", id });
    } catch (e) {
      toast(errorText(e), "error");
      setSaving(false);
    }
  };

  const isPlaylist = info?.kind === "playlist";

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-overlay p-6" onMouseDown={onClose}>
      <div
        className="flex max-h-full w-full max-w-xl flex-col overflow-hidden rounded-2xl bg-raised shadow-2xl"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-6 pt-5 pb-3">
          <h2 className="text-lg font-medium">
            {info ? (isPlaylist ? "Ajouter la playlist" : "Ajouter la vidéo") : "Analyse du lien"}
          </h2>
          <IconButton label="Fermer" onClick={onClose} className="-mr-2">
            <X size={20} />
          </IconButton>
        </div>

        {!info && !error && (
          <div className="flex flex-col items-center gap-3 px-6 py-14 text-sm text-muted">
            <Spinner size={28} />
            Lecture des informations YouTube…
          </div>
        )}

        {error && (
          <div className="px-6 pb-6">
            <div className="flex gap-3 rounded-xl bg-surface p-4 text-sm">
              <AlertCircle size={20} className="shrink-0 text-danger" />
              <div>
                <p className="font-medium">Impossible de lire ce lien</p>
                <p className="mt-1 break-words text-muted">{error}</p>
              </div>
            </div>
            <div className="mt-5 flex justify-end">
              <Button onClick={onClose}>Fermer</Button>
            </div>
          </div>
        )}

        {info && (
          <>
            <div className="min-h-0 flex-1 overflow-y-auto px-6">
              <div className="flex gap-4">
                <Thumb src={info.thumbnail} className="w-44 rounded-xl" />
                <div className="min-w-0">
                  <p className="line-clamp-2 font-medium">{info.title}</p>
                  {info.channel && <p className="mt-1 text-sm text-muted">{info.channel}</p>}
                  <p className="mt-1 text-sm text-muted">
                    {isPlaylist ? plural(info.entries.length, "vidéo") : "Vidéo seule"}
                  </p>
                  {known && (
                    <p className="mt-2 text-xs text-muted">
                      Déjà dans ta bibliothèque : seules les vidéos manquantes seront téléchargées.
                    </p>
                  )}
                </div>
              </div>

              {isPlaylist && (
                <ol className="mt-4 max-h-56 overflow-y-auto rounded-xl border border-border py-1">
                  {info.entries.map((e, i) => (
                    <li key={e.id} className="flex items-center gap-3 px-3 py-1.5 text-sm">
                      <span className="w-6 shrink-0 text-right text-xs text-muted">{i + 1}</span>
                      <span className="truncate">{e.title}</span>
                    </li>
                  ))}
                </ol>
              )}

              <div className="mt-4 divide-y divide-border rounded-xl border border-border">
                {isPlaylist && (
                  <label className="flex cursor-pointer items-center justify-between gap-4 px-4 py-3">
                    <div>
                      <p className="text-sm font-medium">Synchronisation automatique</p>
                      <p className="text-xs text-muted">Télécharge les nouvelles vidéos ajoutées à la playlist</p>
                    </div>
                    <Toggle checked={autoSync} onChange={setAutoSync} label="Synchronisation automatique" />
                  </label>
                )}
                <div className="flex items-center justify-between gap-4 px-4 py-3">
                  <p className="text-sm font-medium">Qualité</p>
                  <Select value={quality} options={QUALITY_WITH_DEFAULT} onChange={setQuality} />
                </div>
              </div>
            </div>

            <div className="flex justify-end gap-2 px-6 pt-5 pb-5">
              <Button variant="ghost" onClick={() => add(false)} disabled={saving}>
                Ajouter sans télécharger
              </Button>
              <Button variant="primary" onClick={() => add(true)} loading={saving}>
                Télécharger
              </Button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
