import { Check, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useDialog } from "../lib/dialog";
import { HOTKEY, core, defaultHotkey, type Settings } from "../lib/tazuna";

// The first run (docs/DESKTOP.md 1.0): what tazuna found and what is left to choose, each choice with its own way there
// and none needed now ("choose later"). Shown once; Cmd-K "Set up tazuna" brings it back.
export const SETUP = "tazuna.setup";

export function Setup({ onSettings, onClose }: { onSettings: () => void; onClose: () => void }) {
  const dialog = useDialog();
  const [st, setSt] = useState<Settings | null>(null);
  const [claude, setClaude] = useState<string | null | undefined>(undefined);
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const offs = [core.onSettings(setSt), core.onSetup((s) => setClaude(s.claude))];
    core.settings().catch(() => {});
    core.setup().catch(() => {});
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") close.current(); };
    window.addEventListener("keydown", esc);
    return () => { offs.forEach((o) => o.then((f) => f())); window.removeEventListener("keydown", esc); };
  }, []);
  const keys = st?.providers.filter((p) => p.key) ?? [];
  const hotkey = localStorage.getItem(HOTKEY) ?? defaultHotkey;
  const row = (done: boolean | undefined, title: string, body: React.ReactNode, action?: React.ReactNode) => (
    <div className="flex items-start gap-3 border-b border-line-soft py-3 last:border-b-0">
      <span className={`mt-0.5 grid h-5 w-5 flex-none place-items-center rounded-full border ${done ? "border-ok text-ok" : "border-line-strong text-faint"}`} aria-hidden="true">
        {done && <Check size={12} strokeWidth={2} />}</span>
      <span className="min-w-0 flex-1"><span className="block text-ink">{title}</span><span className="block text-[12.5px] text-muted">{body}</span></span>
      {action}
    </div>
  );
  const button = "flex-none rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:bg-hover";
  return (
    <div onMouseDown={onClose} className="fixed inset-0 z-30 grid place-items-center bg-ink/15 p-10">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Set up tazuna" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()}
        className="flex max-h-full w-[600px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
        <div className="flex items-center border-b border-line-soft px-5 py-3">
          <span className="flex-1 font-medium">Set up tazuna</span>
          <button onClick={onClose} aria-label="Close" title="Close (Esc)" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="overflow-auto px-5 py-2">
          <div className="py-2 text-[12.5px] text-muted">Nothing here is needed now: each line can wait, and Cmd-K "Set up tazuna" brings this back.</div>
          {row(claude ? true : claude === null ? false : undefined, "Claude on your subscription",
            claude === undefined ? "Looking for the claude CLI…" : claude ? <>Found {claude}. tazuna runs it as it is, with its own login.</>
              : <>The claude CLI is not installed. Install Claude Code, then log in once in a terminal (<code>claude</code>). Without it, tazuna uses the models below.</>,
            claude === null && <button onClick={() => core.openLink("https://docs.claude.com/en/docs/claude-code/setup")} className={button}>How to install</button>)}
          {row(keys.length > 0, "Other models",
            keys.length ? <>Keys saved for {keys.map((p) => p.name).join(", ")}.</> : <>DeepSeek, Mistral, OpenRouter and others need an API key. Add one when you want them.</>,
            <button onClick={onSettings} className={button}>{keys.length ? "Keys" : "Add a key"}</button>)}
          {row(st ? true : undefined, "Who may see your code",
            st?.data_any ? "Any provider, for this session." : "Trusted providers only: the ones you saved a key for and the trusted list. Code never goes elsewhere unless you allow it.",
            <button onClick={onSettings} className={button}>Change</button>)}
          {row(!!hotkey, "Quick chat from anywhere", hotkey ? <><span className="font-mono">{hotkey}</span> brings tazuna up with a new chat.</> : "Off.",
            <button onClick={onSettings} className={button}>Change</button>)}
        </div>
        <div className="flex justify-end gap-2 border-t border-line-soft px-5 py-3">
          <button onClick={onClose} className="rounded-md bg-accent px-3 py-1 font-medium text-on-accent">{claude && keys.length ? "Done" : "Choose later"}</button>
        </div>
      </div>
    </div>
  );
}
