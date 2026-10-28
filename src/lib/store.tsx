import { listen } from "@tauri-apps/api/event";
import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import { api, errorText, Progress, Settings, Theme, ToolsStatus } from "./api";

export type Route =
  | { name: "library" }
  | { name: "playlist"; id: string }
  | { name: "queue" }
  | { name: "settings" };

export interface Toast {
  id: number;
  text: string;
  kind: "info" | "error";
}

interface Store {
  route: Route;
  go: (r: Route) => void;
  settings: Settings | null;
  updateSettings: (patch: Partial<Settings>) => void;
  tools: ToolsStatus | null;
  toolsProgress: { tool: string; downloaded: number; total: number | null } | null;
  refreshTools: () => void;
  progress: Record<number, Progress>;
  /** Bumped every time the backend says videos changed. */
  tick: number;
  queueCount: number;
  toast: (text: string, kind?: Toast["kind"]) => void;
  toasts: Toast[];
  dark: boolean;
  toggleDark: () => void;
}

const Ctx = createContext<Store | null>(null);

export function useStore(): Store {
  const s = useContext(Ctx);
  if (!s) throw new Error("useStore outside provider");
  return s;
}

function systemDark() {
  return matchMedia("(prefers-color-scheme: dark)").matches;
}

function applyTheme(theme: Theme): boolean {
  const dark = theme === "dark" || (theme === "system" && systemDark());
  document.documentElement.classList.toggle("dark", dark);
  try {
    localStorage.setItem("theme", theme);
  } catch {
    // Storage can be unavailable, the theme still applies for this session.
  }
  return dark;
}

export function StoreProvider({ children }: { children: ReactNode }) {
  const [route, setRoute] = useState<Route>({ name: "library" });
  const [settings, setSettings] = useState<Settings | null>(null);
  const [tools, setTools] = useState<ToolsStatus | null>(null);
  const [toolsProgress, setToolsProgress] = useState<Store["toolsProgress"]>(null);
  const [progress, setProgress] = useState<Record<number, Progress>>({});
  const [tick, setTick] = useState(0);
  const [queueCount, setQueueCount] = useState(0);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [dark, setDark] = useState(() => document.documentElement.classList.contains("dark"));
  const saveTimer = useRef<number | undefined>(undefined);

  const toast = useCallback((text: string, kind: Toast["kind"] = "info") => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t, { id, text, kind }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), kind === "error" ? 6000 : 3500);
  }, []);

  const refreshTools = useCallback(() => {
    api.toolsStatus().then(setTools).catch(() => {});
  }, []);

  useEffect(() => {
    api.getSettings().then((s) => {
      setSettings(s);
      setDark(applyTheme(s.theme));
    });
    refreshTools();
    const unlisten = [
      listen<Progress>("video-progress", (e) =>
        setProgress((p) => ({ ...p, [e.payload.id]: e.payload })),
      ),
      listen("videos-changed", () => setTick((t) => t + 1)),
      listen("tools-changed", () => refreshTools()),
      listen<Store["toolsProgress"]>("tools-progress", (e) => setToolsProgress(e.payload)),
    ];
    return () => {
      unlisten.forEach((p) => p.then((f) => f()));
    };
  }, [refreshTools]);

  // Queue badge in the sidebar.
  useEffect(() => {
    api
      .listQueue()
      .then((q) => {
        setQueueCount(q.videos.filter((v) => v.status === "queued" || v.status === "downloading").length);
        const active = new Set(q.videos.filter((v) => v.status === "downloading").map((v) => v.id));
        setProgress((p) => Object.fromEntries(Object.entries(p).filter(([id]) => active.has(Number(id)))));
      })
      .catch(() => {});
  }, [tick]);

  // Follow Windows theme changes while in "system" mode.
  useEffect(() => {
    if (settings?.theme !== "system") return;
    const mq = matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setDark(applyTheme("system"));
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [settings?.theme]);

  const updateSettings = useCallback(
    (patch: Partial<Settings>) => {
      setSettings((prev) => {
        if (!prev) return prev;
        const next = { ...prev, ...patch };
        if (patch.theme) setDark(applyTheme(patch.theme));
        window.clearTimeout(saveTimer.current);
        saveTimer.current = window.setTimeout(() => {
          api.saveSettings(next).catch((e) => toast(errorText(e), "error"));
        }, 400);
        return next;
      });
    },
    [toast],
  );

  const toggleDark = useCallback(() => {
    updateSettings({ theme: dark ? "light" : "dark" });
  }, [dark, updateSettings]);

  const go = useCallback((r: Route) => setRoute(r), []);

  return (
    <Ctx.Provider
      value={{
        route,
        go,
        settings,
        updateSettings,
        tools,
        toolsProgress,
        refreshTools,
        progress,
        tick,
        queueCount,
        toast,
        toasts,
        dark,
        toggleDark,
      }}
    >
      {children}
    </Ctx.Provider>
  );
}
