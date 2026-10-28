# PlaylistSaver

App Windows pour sauvegarder des playlists YouTube en local (VF + VOSTFR) et les regarder dans Jellyfin.

## Fonctionnement

- Colle un lien de playlist ou de vidéo dans la barre du haut.
- Chaque vidéo devient un MKV avec :
  - la piste audio française si YouTube a un doublage (VF), plus la piste originale (VO) ;
  - les sous-titres français intégrés (officiels, ou auto-traduits si l'option est activée).
- Téléchargement dans un dossier local, puis copie sur le NAS et suppression du fichier local.
- Rangement façon série Jellyfin :

```
<Bibliothèque>/
  <Playlist>/
    tvshow.nfo
    poster.jpg
    Season 01/
      <Playlist> - S01E01 - <Titre>.mkv
      <Playlist> - S01E01 - <Titre>.nfo
      <Playlist> - S01E01 - <Titre>-thumb.jpg
```

- Synchro auto des playlists : seules les nouvelles vidéos sont téléchargées.
- File d'attente avec pause / reprise, limite de débit et téléchargements simultanés.
- Scan de la bibliothèque Jellyfin après chaque série de téléchargements (clé API requise).
- Mode jour / nuit / système, et réduction dans la zone de notification.

## Configuration Jellyfin

1. Crée une bibliothèque de type **Séries** qui pointe sur le dossier choisi dans Paramètres → Bibliothèque Jellyfin.
2. Dans les options de la bibliothèque, laisse le lecteur de métadonnées **NFO** activé.
3. Crée une clé API (Tableau de bord → Clés API) et colle-la dans Paramètres → Jellyfin.

## Outils externes

Au premier lancement, l'app télécharge dans `%LOCALAPPDATA%\com.bixth.playlistsaver\bin` :

- `yt-dlp.exe` ;
- `ffmpeg.exe` et `ffprobe.exe` (builds yt-dlp/FFmpeg-Builds) ;
- `deno.exe`, le moteur JavaScript dont yt-dlp a besoin pour YouTube.

Si des téléchargements se mettent à échouer, utilise Paramètres → Outils → Mettre à jour yt-dlp : YouTube casse souvent les anciennes versions.

## Développement

Prérequis : Node 20+, Rust (rustup), Visual Studio Build Tools (C++), WebView2.

```sh
npm install
npm run tauri dev      # dev
npm run tauri build    # installeur NSIS dans src-tauri/target/release/bundle/nsis
```

Structure :

- `src/` : interface React + Tailwind (`views/`, `components/`, `lib/`).
- `src-tauri/src/` : backend Rust.
  - `ytdlp.rs` : lecture des liens, choix des formats VF/VO, téléchargement.
  - `queue.rs` : file d'attente et étapes de traitement d'une vidéo.
  - `library.rs` : NFO, copie sur le NAS, API Jellyfin.
  - `sources.rs` : ajout et synchro des playlists.
  - `db.rs` : base SQLite.
  - `tools.rs` : installation de yt-dlp, ffmpeg et Deno.
