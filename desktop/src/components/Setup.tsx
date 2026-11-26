import { Check, Copy, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useDialog } from "../lib/dialog";
import { keysOf } from "../lib/types";
import { HOTKEY, core, defaultHotkey, type ClaudeSetup, type Settings } from "../lib/tazuna";
import { accentButton, dialogShell, quietButton } from "./Controls";
import { Parts } from "./Sep";

/** Claude Code as the core finds it (core C1), asked for here and again by `check` ("Check again"): undefined until it
 * answers. */
export function useClaude(): [ClaudeSetup | undefined, () => void, boolean] {
  const [s, setS] = useState<ClaudeSetup>();
  const [checking, setChecking] = useState(false);
  useEffect(() => {
    const off = core.onSetup((x) => { setS(x); setChecking(false); });
    core.setup().catch(() => {});
    return () => { off.then((f) => f()); };
  }, []);
  return [s, () => { setChecking(true); core.setup().catch(() => setChecking(false)); }, checking];
}

const CLAUDE_INSTALL = "https://docs.anthropic.com/en/docs/claude-code";

/** Claude Code's state in words, the same in Setup and in Settings → Providers: signed in, signed out (and how to sign in),
 * not installed, or found where the core does not say whether it is signed in (one before 0.1.198). */
export function ClaudeNote({ s }: { s: ClaudeSetup | undefined }) {
  if (s === undefined) return <>Looking for Claude Code…</>;
  if (!s.claude) return <>Claude Code isn't installed. Without it, tazuna uses the other providers' models.</>;
  if (s.claude_login === true) return <span className="text-ok"><Parts items={["Signed in", "runs on your Claude subscription"]} /></span>;
  if (s.claude_login === false) return <>Not signed in<span className="block">Run <code className="font-mono text-[11.5px] text-ink-2">claude auth login</code> in a terminal, then check again.</span></>;
  return <>Found {s.claude}. tazuna runs it as it is, with its own login.</>;
}

/** What to do about it: the install page, or the sign-in command to copy and a check again. The first button is the one
 * Settings opened on Claude's row focuses (`data-sign-in`). */
export function ClaudeButtons({ s, check, checking }: { s: ClaudeSetup | undefined; check: () => void; checking: boolean }) {
  const [copied, setCopied] = useState(false);
  if (s && !s.claude) return <button data-sign-in="Claude" onClick={() => core.openLink(CLAUDE_INSTALL).catch(() => {})} className={`flex-none ${quietButton}`}>Install page</button>;
  if (s?.claude_login !== false) return null;
  const copy = () => navigator.clipboard?.writeText("claude auth login").then(() => { setCopied(true); setTimeout(() => setCopied(false), 1400); }, () => {});
  return <>
    <button data-sign-in="Claude" onClick={copy} title="Copy the command: claude auth login" className={`flex flex-none items-center gap-1.5 ${quietButton}`}>
      {copied ? <Check size={12} strokeWidth={1.75} className="text-ok" /> : <Copy size={12} strokeWidth={1.5} />}{copied ? "Copied" : "Copy"}</button>
    <button onClick={() => !checking && check()} aria-disabled={checking} className={`flex-none ${quietButton} aria-disabled:opacity-60`}>{checking ? "Checking…" : "Check again"}</button>
  </>;
}

// The first run (docs/DESKTOP.md 1.0): what tazuna found and what is left to choose, each choice with its own way there
// and none needed now ("choose later"). Shown once; Cmd-K "Set up tazuna" brings it back.
export const SETUP = "tazuna.setup";

export function Setup({ onSettings, onClose }: { onSettings: () => void; onClose: () => void }) {
  const dialog = useDialog();
  const [st, setSt] = useState<Settings | null>(null);
  const [claude, check, checking] = useClaude();
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const offs = [core.onSettings(setSt)];
    core.settings().catch(() => {});
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
  const button = `flex-none ${quietButton}`;
  return (
    <div onMouseDown={onClose} className="tz-fade fixed inset-0 z-30 grid place-items-center bg-scrim p-10">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Set up tazuna" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()}
        className={dialogShell(600)}>
        <div className="flex items-center border-b border-line-soft px-5 py-3">
          <span className="flex-1 font-medium">Set up tazuna</span>
          <button onClick={onClose} aria-label="Close" title="Close (Esc)" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="overflow-auto px-5 py-2">
          <div className="py-2 text-[12.5px] text-muted">Everything here can wait. {keysOf("⌘K")} › Set up tazuna brings this back.</div>
          {/* ticked only when it is signed in: found but signed out, every message to Claude failed (design review 12) */}
          {row(claude?.claude_login === true, "Claude on your subscription", <ClaudeNote s={claude} />,
            <span className="flex flex-none gap-2"><ClaudeButtons s={claude} check={check} checking={checking} /></span>)}
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
          <button onClick={onClose} className={accentButton}>{claude?.claude_login && keys.length ? "Done" : "Choose later"}</button>
        </div>
      </div>
    </div>
  );
}
