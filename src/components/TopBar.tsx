import { ArrowRight, ListVideo, Moon, Sun, X } from "lucide-react";
import { FormEvent, useState } from "react";
import { useStore } from "../lib/store";
import { IconButton } from "./ui";

export function TopBar({ onSubmitUrl }: { onSubmitUrl: (url: string) => void }) {
  const { dark, toggleDark, go } = useStore();
  const [url, setUrl] = useState("");

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const value = url.trim();
    if (!value) return;
    onSubmitUrl(value);
    setUrl("");
  };

  return (
    <header className="flex h-14 shrink-0 items-center gap-4 bg-bg px-4">
      <button
        onClick={() => go({ name: "library" })}
        className="flex w-56 shrink-0 items-center gap-2 pl-2"
        aria-label="Bibliothèque"
      >
        <span className="flex size-8 items-center justify-center rounded-lg bg-text text-bg">
          <ListVideo size={18} />
        </span>
        <span className="text-lg font-semibold tracking-tight">PlaylistSaver</span>
      </button>

      <form onSubmit={submit} className="mx-auto flex w-full max-w-2xl">
        <div className="relative flex-1">
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="Coller un lien de playlist ou de vidéo YouTube"
            spellCheck={false}
            className="h-10 w-full rounded-l-full border border-border bg-bg pr-10 pl-5 text-[15px] text-text outline-none placeholder:text-muted focus:border-link"
          />
          {url && (
            <button
              type="button"
              onClick={() => setUrl("")}
              className="absolute top-1/2 right-2 -translate-y-1/2 rounded-full p-1 text-muted hover:bg-surface"
              aria-label="Effacer"
            >
              <X size={18} />
            </button>
          )}
        </div>
        <button
          type="submit"
          className="flex h-10 w-16 items-center justify-center rounded-r-full border border-l-0 border-border bg-surface hover:bg-surface-hover"
          aria-label="Ajouter"
          title="Ajouter"
        >
          <ArrowRight size={20} />
        </button>
      </form>

      <div className="flex w-56 shrink-0 justify-end">
        <IconButton label={dark ? "Mode jour" : "Mode nuit"} onClick={toggleDark}>
          {dark ? <Sun size={20} /> : <Moon size={20} />}
        </IconButton>
      </div>
    </header>
  );
}
