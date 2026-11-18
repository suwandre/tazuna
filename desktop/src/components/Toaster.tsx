import { CircleAlert, CircleCheck, Info, X } from "lucide-react";
import { dismiss, useToasts } from "../lib/toast";

/** The snackbars, bottom centre over everything (above the message box). */
export function Toaster() {
  const toasts = useToasts();
  if (!toasts.length) return null;
  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-28 z-50 flex flex-col items-center gap-2">
      {toasts.map((t) => (
        <div key={t.id} role={t.kind === "error" ? "alert" : "status"}
          className="tz-enter pointer-events-auto flex max-w-[520px] items-center gap-2.5 rounded-lg border border-line-strong bg-card py-2 pl-3 pr-2 text-[13px] text-ink shadow-composer">
          {t.kind === "error" ? <CircleAlert size={16} strokeWidth={1.75} className="flex-none text-bad" aria-hidden="true" />
            : t.kind === "ok" ? <CircleCheck size={16} strokeWidth={1.75} className="flex-none text-ok" aria-hidden="true" />
            : <Info size={16} strokeWidth={1.75} className="flex-none text-accent" aria-hidden="true" />}
          <span className="min-w-0 flex-1">{t.text}</span>
          {t.action && <button onClick={() => { dismiss(t.id); t.action!.run(); }}
            className="flex-none rounded-md bg-accent px-2.5 py-1 text-[12px] font-medium text-on-accent">{t.action.label}</button>}
          <button onClick={() => dismiss(t.id)} aria-label="Dismiss" className="flex-none rounded p-1 text-muted hover:bg-hover hover:text-ink"><X size={14} strokeWidth={1.5} /></button>
        </div>
      ))}
    </div>
  );
}
