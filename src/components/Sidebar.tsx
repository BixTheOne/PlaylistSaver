import { Download, Library, Settings } from "lucide-react";
import { ReactNode } from "react";
import { Route, useStore } from "../lib/store";

function Item({
  icon,
  label,
  active,
  badge,
  onClick,
}: {
  icon: ReactNode;
  label: string;
  active: boolean;
  badge?: number;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={`flex h-10 w-full items-center gap-5 rounded-lg px-3 text-sm transition-colors ${
        active ? "bg-surface font-medium" : "hover:bg-surface"
      }`}
    >
      {icon}
      <span className="flex-1 text-left">{label}</span>
      {!!badge && (
        <span className="min-w-5 rounded-full bg-text px-1.5 text-center text-xs leading-5 font-medium text-bg">
          {badge}
        </span>
      )}
    </button>
  );
}

export function Sidebar() {
  const { route, go, queueCount } = useStore();
  const is = (name: Route["name"]) =>
    route.name === name || (name === "library" && route.name === "playlist");

  return (
    <nav className="flex w-60 shrink-0 flex-col gap-1 px-3 pt-2">
      <Item
        icon={<Library size={20} />}
        label="Bibliothèque"
        active={is("library")}
        onClick={() => go({ name: "library" })}
      />
      <Item
        icon={<Download size={20} />}
        label="File d'attente"
        active={is("queue")}
        badge={queueCount}
        onClick={() => go({ name: "queue" })}
      />
      <div className="my-3 border-t border-border" />
      <Item
        icon={<Settings size={20} />}
        label="Paramètres"
        active={is("settings")}
        onClick={() => go({ name: "settings" })}
      />
    </nav>
  );
}
