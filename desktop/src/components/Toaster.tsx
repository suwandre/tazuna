import { CircleAlert, CircleCheck, Info, X } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useDialog } from "../lib/dialog";
import { dismiss, hold, useToasts, type Toast } from "../lib/toast";
import { progressText, useUpdate } from "../lib/update";
import { accentButton, dialogShell, quietButton } from "./Controls";

const icon = (t: Toast, size: number) => t.kind === "error" ? <CircleAlert size={size} strokeWidth={1.75} className="flex-none text-bad" aria-hidden="true" />
  : t.kind === "ok" ? <CircleCheck size={size} strokeWidth={1.75} className="flex-none text-ok" aria-hidden="true" />
  : <Info size={size} strokeWidth={1.75} className="flex-none text-live" aria-hidden="true" />;

/** Where the snackbars were dragged to, from their place at the bottom centre (px). */
const MOVED = "tazuna.toast-offset";
const movedAt = (): [number, number] => {
  try { const v = JSON.parse(localStorage.getItem(MOVED) ?? "null"); return Array.isArray(v) && v.length === 2 && v.every(Number.isFinite) ? [v[0], v[1]] : [0, 0]; } catch { return [0, 0]; }
};

/** The snackbars, bottom centre over everything (above the message box), or where the user dragged them (the user,
 * 2026-11-27: "tazuna desktop 0.1.136 is out" sat over the work, and closing it lost the reminder): pressed anywhere but on
 * a button and moved, they follow the pointer, inside the window, and the next ones come up there too. The newest modal
 * one in the middle. */
export function Toaster() {
  const toasts = useToasts();
  const bar = toasts.filter((t) => !t.modal), modal = toasts.filter((t) => t.modal).at(-1);
  const update = useUpdate();
  const stack = useRef<HTMLDivElement>(null);
  const [off, setOff] = useState(movedAt);
  // the offset on screen now (the stack's measured place has it in)
  const now = useRef(off);
  now.current = off;
  /** The stack's place without the offset: measured with the offset on screen now taken out. */
  const home = () => { const r = stack.current?.getBoundingClientRect(); return r && { left: r.left - now.current[0], top: r.top - now.current[1], w: r.width, h: r.height }; };
  /** An offset kept inside the window (8 px from each edge) for the stack at `h`. */
  const inside = ([x, y]: [number, number], h = home()): [number, number] => {
    if (!h) return [x, y];
    const pad = 8;
    return [Math.round(Math.min(Math.max(x, pad - h.left), innerWidth - pad - h.left - h.w)), Math.round(Math.min(Math.max(y, pad - h.top), innerHeight - pad - h.top - h.h))];
  };
  const shown = bar.length > 0 || !!update.installing;
  // the snackbars wait while the pointer is on them or drags them (one gone mid-drag lost the drag and its new place)
  const [over, setOver] = useState(false), [dragging, setDragging] = useState(false);
  // back inside the window when it gets smaller or the stack changes size (a snackbar more or replaced, a longer one)
  useLayoutEffect(() => {
    if (!shown) return;
    const fit = () => setOff((o) => { const n = inside(o); return n[0] === o[0] && n[1] === o[1] ? o : n; });
    fit();
    const r = new ResizeObserver(fit);
    if (stack.current) r.observe(stack.current);
    addEventListener("resize", fit);
    // (gone from under the pointer: no pointerleave comes)
    return () => { r.disconnect(); removeEventListener("resize", fit); setOver(false); };
  }, [shown]);
  useEffect(() => { hold(over || dragging); }, [over, dragging]);
  useEffect(() => () => hold(false), []);
  // a drag starts once the pointer has moved 4 px: a click, a double or triple click (selecting an error's text to copy it)
  // stays the page's own
  const drag = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
    const x0 = e.clientX, y0 = e.clientY, start = now.current;
    let at = start, moving = false;
    const quiet = (s: Event) => { if (moving) s.preventDefault(); };
    const move = (m: PointerEvent) => {
      if (!moving) {
        if (Math.hypot(m.clientX - x0, m.clientY - y0) < 4) return;
        moving = true; setDragging(true); getSelection()?.removeAllRanges();
      }
      // measured each time: a snackbar coming or going mid-drag changes the stack's size
      at = inside([start[0] + m.clientX - x0, start[1] + m.clientY - y0]); setOff(at);
    };
    const up = () => {
      removeEventListener("pointermove", move); removeEventListener("pointerup", up); removeEventListener("pointercancel", up); removeEventListener("selectstart", quiet);
      if (moving) { setDragging(false); localStorage.setItem(MOVED, JSON.stringify(at)); }
    };
    addEventListener("pointermove", move); addEventListener("pointerup", up); addEventListener("pointercancel", up); addEventListener("selectstart", quiet);
  };
  const grab = `touch-none ${dragging ? "cursor-grabbing select-none" : "cursor-grab"}`;
  return <>
    {shown && <div className="pointer-events-none fixed inset-x-0 bottom-28 z-50 flex justify-center">
     <div ref={stack} onPointerEnter={() => setOver(true)} onPointerLeave={() => setOver(false)} className="flex flex-col items-center gap-2" style={{ transform: `translate(${off[0]}px, ${off[1]}px)` }}>
      {/* an update on its way: its progress until the restart (no close: it cannot be stopped halfway) */}
      {update.installing && (
        <div role="status" onPointerDown={drag} className={`tz-enter pointer-events-auto w-[360px] rounded-lg border border-line-strong bg-card px-3 py-2.5 text-[13px] text-ink tz-float ${grab}`}>
          <div className="flex items-center gap-2.5"><Info size={16} strokeWidth={1.75} className="flex-none text-live" aria-hidden="true" />
            <span className="min-w-0 flex-1 truncate">{progressText(update)}</span></div>
          <div className="mt-2 h-1 overflow-hidden rounded bg-line"><i className={`block h-full rounded bg-live transition-[width] duration-200 ${update.stage === "installing" || !update.total ? "animate-pulse" : ""}`}
            style={{ width: `${update.stage === "installing" ? 100 : update.total ? Math.min(100, (100 * (update.got ?? 0)) / update.total) : 30}%` }} /></div>
        </div>
      )}
      {bar.map((t) => (
        <div key={t.id} role={t.kind === "error" ? "alert" : "status"} onPointerDown={drag}
          className={`tz-enter pointer-events-auto flex max-w-[520px] items-center gap-2.5 rounded-lg border border-line-strong bg-card py-2 pl-3 pr-2 text-[13px] text-ink tz-float ${grab}`}>
          {icon(t, 16)}
          {/* selectable (a double or triple click), an error's words to copy; the app's text is not otherwise */}
          <span className={`min-w-0 flex-1 ${dragging ? "" : "select-text"}`}>{t.text}</span>
          {t.action && <button onClick={() => { dismiss(t.id); t.action!.run(); }}
            className={`flex-none ${accentButton}`}>{t.action.label}</button>}
          <button onClick={() => dismiss(t.id)} aria-label="Dismiss" className="flex-none rounded p-1 text-muted hover:bg-hover hover:text-ink"><X size={14} strokeWidth={1.5} /></button>
        </div>
      ))}
     </div>
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
        tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className={`${dialogShell(420)} p-5`}>
        <div className="flex items-start gap-3">
          <span className="mt-0.5">{icon(t, 20)}</span>
          <div className="min-w-0 flex-1">
            <div id={`toast-${t.id}`} className="text-[14px] font-semibold text-ink">{t.text}</div>
            {t.detail && <div id={`toast-${t.id}-detail`} className="mt-1 whitespace-pre-wrap text-[12.5px] leading-relaxed text-muted">{t.detail}</div>}
          </div>
          <button onClick={close} aria-label="Close" title="Close (Esc)" className="-mr-1 -mt-1 flex-none rounded p-1 text-muted hover:text-ink"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="mt-5 flex justify-end gap-2">
          <button autoFocus={!t.action} onClick={close} className={quietButton}>{t.action ? "Later" : "OK"}</button>
          {t.action && <button autoFocus onClick={() => { close(); t.action!.run(); }}
            className={accentButton}>{t.action.label}</button>}
        </div>
      </div>
    </div>
  );
}
