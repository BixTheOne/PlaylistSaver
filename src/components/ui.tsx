import { Loader2 } from "lucide-react";
import { ButtonHTMLAttributes, ReactNode } from "react";
import { VideoStatus } from "../lib/api";
import { duration } from "../lib/format";

type Variant = "primary" | "secondary" | "ghost" | "danger";

const VARIANTS: Record<Variant, string> = {
  // Inverted pill, like the main action buttons on YouTube.
  primary: "bg-text text-bg hover:opacity-85",
  secondary: "bg-surface text-text hover:bg-surface-hover",
  ghost: "text-text hover:bg-surface",
  danger: "bg-surface text-danger hover:bg-surface-hover",
};

export function Button({
  variant = "secondary",
  icon,
  loading,
  className = "",
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant;
  icon?: ReactNode;
  loading?: boolean;
}) {
  return (
    <button
      {...props}
      disabled={props.disabled || loading}
      className={`inline-flex h-9 items-center justify-center gap-2 rounded-full px-4 text-sm font-medium whitespace-nowrap transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${VARIANTS[variant]} ${className}`}
    >
      {loading ? <Loader2 size={18} className="animate-spin" /> : icon}
      {children}
    </button>
  );
}

export function IconButton({
  label,
  className = "",
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return (
    <button
      {...props}
      title={label}
      aria-label={label}
      className={`inline-flex size-10 shrink-0 items-center justify-center rounded-full text-text transition-colors hover:bg-surface disabled:opacity-40 ${className}`}
    >
      {children}
    </button>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={`relative h-5 w-9 shrink-0 rounded-full transition-colors ${checked ? "bg-text" : "bg-surface-hover"}`}
    >
      <span
        className={`absolute top-0.5 left-0.5 size-4 rounded-full shadow transition-transform ${checked ? "translate-x-4 bg-bg" : "bg-white"}`}
      />
    </button>
  );
}

export function Select<T extends string | number>({
  value,
  options,
  onChange,
  className = "",
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  className?: string;
}) {
  return (
    <select
      value={String(value)}
      onChange={(e) => {
        const opt = options.find((o) => String(o.value) === e.target.value);
        if (opt) onChange(opt.value);
      }}
      className={`h-9 cursor-pointer rounded-lg border border-border bg-bg px-3 text-sm text-text outline-none focus:border-text ${className}`}
    >
      {options.map((o) => (
        <option key={String(o.value)} value={String(o.value)}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

export function TextInput({
  value,
  onChange,
  placeholder,
  type = "text",
  className = "",
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  type?: string;
  className?: string;
}) {
  return (
    <input
      type={type}
      value={value}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
      spellCheck={false}
      className={`h-9 min-w-0 rounded-lg border border-border bg-bg px-3 text-sm text-text outline-none placeholder:text-muted focus:border-text ${className}`}
    />
  );
}

export function Thumb({
  src,
  seconds,
  className = "",
  children,
}: {
  src: string | null;
  seconds?: number | null;
  className?: string;
  children?: ReactNode;
}) {
  return (
    <div className={`relative aspect-video shrink-0 overflow-hidden bg-surface ${className}`}>
      {src && <img src={src} alt="" loading="lazy" className="size-full object-cover" draggable={false} />}
      {seconds != null && (
        <span className="absolute right-1.5 bottom-1.5 rounded bg-black/80 px-1 py-px text-xs font-medium text-white">
          {duration(seconds)}
        </span>
      )}
      {children}
    </div>
  );
}

export function ProgressBar({ value, className = "" }: { value: number; className?: string }) {
  return (
    <div className={`h-1 overflow-hidden rounded-full bg-surface-hover ${className}`}>
      <div
        className="h-full rounded-full bg-accent transition-[width] duration-300"
        style={{ width: `${Math.max(0, Math.min(100, value))}%` }}
      />
    </div>
  );
}

const STATUS: Record<VideoStatus, { label: string; className: string }> = {
  new: { label: "Non téléchargée", className: "text-muted" },
  queued: { label: "En attente", className: "text-muted" },
  downloading: { label: "Téléchargement", className: "text-text" },
  paused: { label: "En pause", className: "text-warning" },
  done: { label: "Téléchargée", className: "text-success" },
  error: { label: "Erreur", className: "text-danger" },
};

export function StatusLabel({ status }: { status: VideoStatus }) {
  const s = STATUS[status] ?? STATUS.new;
  return <span className={`text-xs font-medium ${s.className}`}>{s.label}</span>;
}

export function EmptyState({
  icon,
  title,
  text,
  action,
}: {
  icon: ReactNode;
  title: string;
  text?: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center px-6 py-24 text-center">
      <div className="mb-4 text-muted">{icon}</div>
      <h2 className="text-lg font-medium">{title}</h2>
      {text && <p className="mt-1 max-w-md text-sm text-muted">{text}</p>}
      {action && <div className="mt-5">{action}</div>}
    </div>
  );
}

export function Spinner({ size = 20 }: { size?: number }) {
  return <Loader2 size={size} className="animate-spin text-muted" />;
}
