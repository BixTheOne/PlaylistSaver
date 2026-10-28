import { useCallback, useState } from "react";
import { AddDialog } from "./components/AddDialog";
import { LibraryDirBanner, Toasts, ToolsBanner } from "./components/Banners";
import { Sidebar } from "./components/Sidebar";
import { TopBar } from "./components/TopBar";
import { StoreProvider, useStore } from "./lib/store";
import { LibraryView } from "./views/LibraryView";
import { PlaylistView } from "./views/PlaylistView";
import { QueueView } from "./views/QueueView";
import { SettingsView } from "./views/SettingsView";

function Shell() {
  const { route } = useStore();
  const [addUrl, setAddUrl] = useState<string | null>(null);
  const closeAdd = useCallback(() => setAddUrl(null), []);

  return (
    <div className="flex h-full flex-col">
      <TopBar onSubmitUrl={setAddUrl} />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <main className="min-w-0 flex-1 overflow-y-auto">
          <ToolsBanner />
          {route.name !== "settings" && <LibraryDirBanner />}
          {route.name === "library" && <LibraryView />}
          {route.name === "playlist" && <PlaylistView id={route.id} />}
          {route.name === "queue" && <QueueView />}
          {route.name === "settings" && <SettingsView />}
        </main>
      </div>
      {addUrl && <AddDialog url={addUrl} onClose={closeAdd} />}
      <Toasts />
    </div>
  );
}

export default function App() {
  return (
    <StoreProvider>
      <Shell />
    </StoreProvider>
  );
}
