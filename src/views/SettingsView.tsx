import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { open } from "@tauri-apps/plugin-dialog";
import { Monitor, Moon, Sun } from "lucide-react";
import { ReactNode, useEffect, useState } from "react";
import { api, errorText, Settings, Theme, ToolsVersions } from "../lib/api";
import { QUALITY_OPTIONS } from "../lib/format";
import { useStore } from "../lib/store";
import { Button, Select, Spinner, TextInput, Toggle } from "../components/ui";

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mt-8">
      <h2 className="mb-3 text-base font-medium">{title}</h2>
      <div className="divide-y divide-border rounded-xl border border-border">{children}</div>
    </section>
  );
}

function Row({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 px-4 py-3.5">
      <div className="min-w-0">
        <p className="text-sm font-medium">{label}</p>
        {hint && <p className="mt-0.5 text-xs text-muted">{hint}</p>}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

function ThemePicker({ value, onChange }: { value: Theme; onChange: (t: Theme) => void }) {
  const options: { value: Theme; label: string; icon: ReactNode }[] = [
    { value: "light", label: "Jour", icon: <Sun size={16} /> },
    { value: "dark", label: "Nuit", icon: <Moon size={16} /> },
    { value: "system", label: "Système", icon: <Monitor size={16} /> },
  ];
  return (
    <div className="flex rounded-full bg-surface p-1">
      {options.map((o) => (
        <button
          key={o.value}
          onClick={() => onChange(o.value)}
          className={`flex h-7 items-center gap-1.5 rounded-full px-3 text-sm transition-colors ${
            value === o.value ? "bg-bg font-medium shadow-sm" : "text-muted hover:text-text"
          }`}
        >
          {o.icon}
          {o.label}
        </button>
      ))}
    </div>
  );
}

function FolderInput({
  value,
  placeholder,
  onChange,
}: {
  value: string;
  placeholder: string;
  onChange: (v: string) => void;
}) {
  const browse = async () => {
    const picked = await open({ directory: true, defaultPath: value || undefined });
    if (typeof picked === "string") onChange(picked);
  };
  return (
    <>
      <TextInput value={value} onChange={onChange} placeholder={placeholder} className="w-80" />
      <Button onClick={browse}>Parcourir</Button>
    </>
  );
}

export function SettingsView() {
  const { settings, updateSettings, tools, toast } = useStore();
  const [autostart, setAutostart] = useState(false);
  const [versions, setVersions] = useState<ToolsVersions | null>(null);
  const [updating, setUpdating] = useState(false);
  const [testing, setTesting] = useState(false);

  useEffect(() => {
    isEnabled().then(setAutostart).catch(() => {});
  }, []);

  useEffect(() => {
    if (tools?.ready) api.toolsVersions().then(setVersions).catch(() => {});
  }, [tools?.ready]);

  if (!settings) {
    return (
      <div className="flex justify-center py-24">
        <Spinner />
      </div>
    );
  }

  const set = <K extends keyof Settings>(key: K) => (value: Settings[K]) => updateSettings({ [key]: value });

  const toggleAutostart = async (on: boolean) => {
    try {
      await (on ? enable() : disable());
      setAutostart(on);
    } catch (e) {
      toast(errorText(e), "error");
    }
  };

  const updateYtdlp = async () => {
    setUpdating(true);
    try {
      toast(await api.updateYtdlp());
      setVersions(await api.toolsVersions());
    } catch (e) {
      toast(errorText(e), "error");
    } finally {
      setUpdating(false);
    }
  };

  const testJellyfin = async () => {
    setTesting(true);
    try {
      const info = await api.testJellyfin(settings.jellyfinUrl, settings.jellyfinApiKey);
      toast(`Connecté à ${info.serverName}${info.version ? ` (${info.version})` : ""}`);
    } catch (e) {
      toast(errorText(e), "error");
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="max-w-4xl px-6 pt-4 pb-16">
      <h1 className="text-2xl font-bold">Paramètres</h1>

      <Group title="Général">
        <Row label="Thème">
          <ThemePicker value={settings.theme} onChange={set("theme")} />
        </Row>
        <Row label="Réduire dans la zone de notification" hint="La fenêtre se cache à la fermeture, les téléchargements et la synchro continuent.">
          <Toggle checked={settings.closeToTray} onChange={set("closeToTray")} />
        </Row>
        <Row label="Lancer au démarrage de Windows" hint="Démarre réduit pour synchroniser les playlists en arrière-plan.">
          <Toggle checked={autostart} onChange={toggleAutostart} />
        </Row>
      </Group>

      <Group title="Dossiers">
        <Row label="Bibliothèque Jellyfin" hint="Dossier de la bibliothèque « Séries » sur le NAS, ex. \\NAS\media\YouTube">
          <FolderInput value={settings.libraryDir} placeholder="\\NAS\media\YouTube" onChange={set("libraryDir")} />
        </Row>
        <Row label="Dossier temporaire" hint="Les vidéos sont téléchargées ici puis copiées sur le NAS.">
          <FolderInput value={settings.tempDir} placeholder={tools?.defaultTemp ?? ""} onChange={set("tempDir")} />
        </Row>
        <Row label="Nom du dossier des vidéos seules">
          <TextInput value={settings.singlesTitle} onChange={set("singlesTitle")} className="w-64" />
        </Row>
      </Group>

      <Group title="Téléchargement">
        <Row label="Qualité maximale" hint="La 4K de YouTube est en VP9/AV1 : Jellyfin peut avoir à la transcoder.">
          <Select value={settings.maxHeight} options={QUALITY_OPTIONS} onChange={set("maxHeight")} />
        </Row>
        <Row label="Piste audio par défaut" hint="Quand une VF existe, les deux pistes sont gardées dans le fichier.">
          <Select
            value={settings.defaultAudio}
            options={[
              { value: "vf", label: "VF (français)" },
              { value: "vo", label: "VO (originale)" },
            ]}
            onChange={set("defaultAudio")}
          />
        </Row>
        <Row
          label="Sous-titres français automatiques"
          hint="Utilise les sous-titres auto-générés ou auto-traduits de YouTube quand il n'y en a pas d'officiels."
        >
          <Toggle checked={settings.autoSubs} onChange={set("autoSubs")} />
        </Row>
        <Row label="Téléchargements simultanés">
          <Select
            value={settings.concurrency}
            options={[1, 2, 3, 4].map((n) => ({ value: n, label: String(n) }))}
            onChange={set("concurrency")}
          />
        </Row>
        <Row label="Limite de débit" hint="Ex. 5M pour 5 Mo/s. Vide = pas de limite.">
          <TextInput value={settings.rateLimit} onChange={set("rateLimit")} placeholder="Illimité" className="w-32" />
        </Row>
      </Group>

      <Group title="Synchronisation">
        <Row label="Vérifier les playlists" hint="Pour les playlists avec la synchro auto activée.">
          <Select
            value={settings.syncIntervalHours}
            options={[
              { value: 0, label: "Jamais" },
              { value: 1, label: "Toutes les heures" },
              { value: 6, label: "Toutes les 6 heures" },
              { value: 12, label: "Toutes les 12 heures" },
              { value: 24, label: "Une fois par jour" },
            ]}
            onChange={set("syncIntervalHours")}
          />
        </Row>
      </Group>

      <Group title="Jellyfin">
        <Row label="Adresse du serveur">
          <TextInput
            value={settings.jellyfinUrl}
            onChange={set("jellyfinUrl")}
            placeholder="http://192.168.1.10:8096"
            className="w-80"
          />
        </Row>
        <Row label="Clé API" hint="Tableau de bord Jellyfin → Clés API. Sert à rafraîchir la bibliothèque après chaque téléchargement.">
          <TextInput
            type="password"
            value={settings.jellyfinApiKey}
            onChange={set("jellyfinApiKey")}
            className="w-80"
          />
        </Row>
        <Row label="Connexion">
          <Button onClick={testJellyfin} loading={testing}>
            Tester
          </Button>
          <Button
            onClick={() =>
              api
                .refreshJellyfin()
                .then(() => toast("Scan de la bibliothèque lancé"))
                .catch((e) => toast(errorText(e), "error"))
            }
          >
            Scanner maintenant
          </Button>
        </Row>
      </Group>

      <Group title="Outils">
        <Row
          label="yt-dlp"
          hint={versions?.ytdlp ? `Version ${versions.ytdlp}` : tools?.ready ? "…" : "Non installé"}
        >
          <Button onClick={updateYtdlp} loading={updating} disabled={!tools?.ready}>
            Mettre à jour
          </Button>
        </Row>
        <Row label="ffmpeg" hint={versions?.ffmpeg ?? (tools?.ready ? "…" : "Non installé")}>
          <span />
        </Row>
        <Row label="Deno" hint={versions?.deno ? `Version ${versions.deno}` : tools?.ready ? "…" : "Non installé"}>
          <span />
        </Row>
      </Group>
    </div>
  );
}
