import { CircleAlert, CircleCheck, Info, X } from "lucide-react";
import { useEffect } from "react";
import { useDialog } from "../lib/dialog";
import { dismiss, useToasts, type Toast } from "../lib/toast";
import { progressText, useUpdate } from "../lib/update";

const icon = (t: Toast, size: number) => t.kind === "error" ? <CircleAlert size={size} strokeWidth={1.75} className="flex-none text-bad" aria-hidden="true" />
  : t.kind === "ok" ? <CircleCheck size={size} strokeWidth={1.75} className="flex-none text-ok" aria-hidden="true" />
  : <Info size={size} strokeWidth={1.75} className="flex-none text-accent" aria-hidden="true" />;

/** The snackbars, bottom centre over everything (above the message box); the newest modal one in the middle. */
export function Toaster() {
  const toasts = useToasts();
  const bar = toasts.filter((t) => !t.modal), modal = toasts.filter((t) => t.modal).at(-1);
  const update = useUpdate();
  return <>
    {(bar.length > 0 || update.installing) && <div className="pointer-events-none fixed inset-x-0 bottom-28 z-50 flex flex-col items-center gap-2">
      {/* an update on its way: its progress until the restart (no close: it cannot be stopped halfway) */}
      {update.installing && (
        <div role="status" className="tz-enter pointer-events-auto w-[360px] rounded-lg border border-line-strong bg-card px-3 py-2.5 text-[13px] text-ink tz-float">
          <div className="flex items-center gap-2.5"><Info size={16} strokeWidth={1.75} className="flex-none text-accent" aria-hidden="true" />
            <span className="min-w-0 flex-1 truncate">{progressText(update)}</span></div>
          <div className="mt-2 h-1 overflow-hidden rounded bg-line"><i className={`block h-full rounded bg-accent transition-[width] duration-200 ${update.stage === "installing" || !update.total ? "animate-pulse" : ""}`}
            style={{ width: `${update.stage === "installing" ? 100 : update.total ? Math.min(100, (100 * (update.got ?? 0)) / update.total) : 30}%` }} /></div>
        </div>
      )}
      {bar.map((t) => (
        <div key={t.id} role={t.kind === "error" ? "alert" : "status"}
          className="tz-enter pointer-events-auto flex max-w-[520px] items-center gap-2.5 rounded-lg border border-line-strong bg-card py-2 pl-3 pr-2 text-[13px] text-ink tz-float">
          {icon(t, 16)}
          <span className="min-w-0 flex-1">{t.text}</span>
          {t.action && <button onClick={() => { dismiss(t.id); t.action!.run(); }}
            className="flex-none rounded-md bg-accent px-2.5 py-1 text-[12px] font-medium text-on-accent">{t.action.label}</button>}
          <button onClick={() => dismiss(t.id)} aria-label="Dismiss" className="flex-none rounded p-1 text-muted hover:bg-hover hover:text-ink"><X size={14} strokeWidth={1.5} /></button>
        </div>
      ))}
    </div>}
    {modal && <ToastDialog key={modal.id} t={modal} />}
  </>;
}

/** A modal snackbar: in the middle over a dimmed window; Esc, × or a click outside closes it alone (Settings under it
 * stays open), and focus goes back where it was. */
function ToastDialog({ t }: { t: Toast }) {
  const dialog = useDialog();
  const close = () => dismiss(t.id);
  useEffect(() => {
    // caught before the panels' own Escape (theirs listen on the way back up)
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopPropagation(); e.preventDefault(); dismiss(t.id); } };
    window.addEventListener("keydown", esc, true);
    return () => window.removeEventListener("keydown", esc, true);
  }, [t.id]);
  return (
    <div onMouseDown={close} className="tz-fade fixed inset-0 z-50 grid place-items-center bg-scrim p-10">
      <div ref={dialog} role="alertdialog" aria-modal="true" aria-labelledby={`toast-${t.id}`} aria-describedby={t.detail ? `toast-${t.id}-detail` : undefined}
        tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className="tz-dialog w-[420px] max-w-[calc(100vw-5rem)] rounded-xl border border-line-strong bg-card p-5 shadow-composer">
        <div className="flex items-start gap-3">
          <span className="mt-0.5">{icon(t, 20)}</span>
          <div className="min-w-0 flex-1">
            <div id={`toast-${t.id}`} className="text-[14px] font-semibold text-ink">{t.text}</div>
            {t.detail && <div id={`toast-${t.id}-detail`} className="mt-1 whitespace-pre-wrap text-[12.5px] leading-relaxed text-muted">{t.detail}</div>}
          </div>
          <button onClick={close} aria-label="Close" title="Close (Esc)" className="-mr-1 -mt-1 flex-none rounded p-1 text-muted hover:text-ink"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="mt-5 flex justify-end gap-2">
          <button autoFocus={!t.action} onClick={close} className="rounded-md border border-line px-3 py-1 text-[12.5px] text-ink">{t.action ? "Later" : "OK"}</button>
          {t.action && <button autoFocus onClick={() => { close(); t.action!.run(); }}
            className="rounded-md bg-accent px-3 py-1 text-[12.5px] font-medium text-on-accent">{t.action.label}</button>}
        </div>
      </div>
    </div>
  );
}
