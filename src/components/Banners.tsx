import { AlertCircle, CheckCircle2, FolderOpen } from "lucide-react";
import { api } from "../lib/api";
import { useStore } from "../lib/store";
import { Button, ProgressBar, Spinner } from "./ui";

const TOOL_NAMES: Record<string, string> = { "yt-dlp": "yt-dlp", ffmpeg: "ffmpeg", deno: "Deno" };

export function ToolsBanner() {
  const { tools, toolsProgress, refreshTools } = useStore();
  if (!tools || tools.ready) return null;

  if (tools.install.error) {
    return (
      <div className="mx-6 mt-2 flex items-center gap-3 rounded-xl bg-surface px-4 py-3 text-sm">
        <AlertCircle size={20} className="shrink-0 text-danger" />
        <div className="min-w-0 flex-1">
          <p className="font-medium">Installation des outils impossible</p>
          <p className="truncate text-muted">{tools.install.error}</p>
        </div>
        <Button onClick={() => api.installTools().finally(refreshTools)}>Réessayer</Button>
      </div>
    );
  }

  const pct =
    toolsProgress?.total && toolsProgress.total > 0
      ? (toolsProgress.downloaded / toolsProgress.total) * 100
      : 0;
  return (
    <div className="mx-6 mt-2 rounded-xl bg-surface px-4 py-3 text-sm">
      <div className="flex items-center gap-3">
        <Spinner size={18} />
        <p className="flex-1">
          Première installation des outils
          {toolsProgress ? ` : ${TOOL_NAMES[toolsProgress.tool] ?? toolsProgress.tool}` : ""}…
        </p>
        {pct > 0 && <span className="text-muted">{Math.round(pct)} %</span>}
      </div>
      <ProgressBar value={pct} className="mt-2" />
    </div>
  );
}

export function LibraryDirBanner() {
  const { settings, go } = useStore();
  if (!settings || settings.libraryDir.trim()) return null;
  return (
    <div className="mx-6 mt-2 flex items-center gap-3 rounded-xl bg-surface px-4 py-3 text-sm">
      <FolderOpen size={20} className="shrink-0" />
      <p className="flex-1">Choisis le dossier de ta bibliothèque Jellyfin (partage du NAS) avant de télécharger.</p>
      <Button variant="primary" onClick={() => go({ name: "settings" })}>
        Configurer
      </Button>
    </div>
  );
}

export function Toasts() {
  const { toasts } = useStore();
  return (
    <div className="pointer-events-none fixed bottom-6 left-6 z-50 flex flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          className="flex max-w-md items-center gap-3 rounded-lg bg-text px-4 py-3 text-sm text-bg shadow-lg"
        >
          {t.kind === "error" ? (
            <AlertCircle size={18} className="shrink-0" />
          ) : (
            <CheckCircle2 size={18} className="shrink-0" />
          )}
          <span className="break-words">{t.text}</span>
        </div>
      ))}
    </div>
  );
}
