import { ExternalLink, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useDialog } from "../lib/dialog";
import { checkUpdate, installUpdate, useUpdate } from "../lib/update";
import { safeMode } from "../lib/plugins";
import { HOTKEY, core, defaultHotkey, type GalleryItem, type Hook, type Mcp, type Settings } from "../lib/tazuna";
import { dryRun, summary, widgets, type UserWidget } from "../lib/widgets";

// Settings: provider keys (saved by tazuna, never shown again), the data policy for this session, the router. The
// terminal's /login, /data and /route in one page.
type PluginProps = {
  items?: UserWidget[]; onEnable?: (w: UserWidget, on: boolean) => void; onEdit?: (id: string) => void; onRemove?: (id: string) => void;
  /** a panel or plugin came in (a file, the gallery): the list again */
  onChanged?: () => void;
};

export function SettingsView({ onClose, items = [], onEnable, onEdit, onRemove, onChanged }: { onClose: () => void } & PluginProps) {
  const [st, setSt] = useState<Settings | null>(null);
  const dialog = useDialog();
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [askAny, setAskAny] = useState(false);
  const [keysNow, setKeysNow] = useState(() => localStorage.getItem(HOTKEY) ?? defaultHotkey);
  const [hotkeyNote, setHotkeyNote] = useState("");
  const [cap, setCap] = useState<string | null>(null);
  const [mcp, setMcp] = useState<Mcp | null>(null);
  const [mcpState, setMcpState] = useState<Record<string, { ok?: boolean; text: string }>>({});
  const [adding, setAdding] = useState({ name: "", target: "" });
  const [hook, setHook] = useState<Hook | null>(null);
  // the app's own version beside its core's: "tazuna 0.1.173" alone read as the terminal's
  const [appVersion, setAppVersion] = useState("");
  useEffect(() => { getVersion().then(setAppVersion).catch(() => {}); }, []);
  // updates from here (until 0.1.58 only the sidebar's row, when one was found, and Cmd-K)
  const update = useUpdate();
  const [checking, setChecking] = useState(false);
  // a plugin read before it is enabled (its code and what it would register, from one read); sharing and the gallery
  const [reading, setReading] = useState<{ w: UserWidget; code: string; print: string; would: string } | null>(null);
  const [plugNote, setPlugNote] = useState("");
  const [gallery, setGallery] = useState<{ items?: GalleryItem[]; error?: string } | null>(null);
  // the gallery entries asked for and not yet in: each answer is taken once (a second click or listener adds no copy)
  const asked = useRef(new Set<string>());
  const [, setAskedN] = useState(0);
  const read = async (w: UserWidget) => {
    const c = await widgets.code(w.id).catch(() => null);
    if (!c) { setPlugNote(`Could not read ${w.title}'s code.`); return; }
    const r = await dryRun(w.id);
    // what a trial run saw it register: the plugin's own behaviour there, not a promise (it can act differently later)
    setReading({ w, code: c.code, print: c.print, would: r.error ? `In a trial run it failed: ${r.error}` : `In a trial run it registered: ${summary(r.rec) || "nothing"}` });
  };
  const arrived = (id: string | null, from: string) => {
    if (!id) return;
    onChanged?.();
    setPlugNote(`Added ${id} from ${from}. A plugin stays off until you read its code and enable it; a panel shows at once, sandboxed.`);
  };
  const [hookCmd, setHookCmd] = useState<string | null>(null);
  const canAdd = !!adding.name.trim() && !!adding.target.trim();
  const addServer = () => {
    const name = adding.name.trim();
    setMcpState((all) => ({ ...all, [name]: { text: "connecting…" } }));
    core.mcpAdd(name, adding.target.trim()).then(() => setAdding({ name: "", target: "" }));
  };
  const saveHotkey = () => core.hotkey(keysNow.trim())
    .then(() => { localStorage.setItem(HOTKEY, keysNow.trim()); setHotkeyNote(keysNow.trim() ? "Saved." : "Off."); })
    .catch((e) => setHotkeyNote(`Not set: ${e}. Another app may hold it.`));
  const refresh = () => core.settings().catch(() => {});
  // asked once when opened (the window re-renders on every streamed view, with a new onClose each time)
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const off = core.onSettings(setSt);
    const offMcp = core.onMcp(setMcp), offStatus = core.onMcpStatus((m) => setMcpState((all) => ({ ...all, [m.name]: m }))), offHook = core.onHook(setHook);
    const offGallery = core.onGallery(setGallery), offItem = core.onGalleryItem((g) => {
      if (g.file && !asked.current.delete(g.file)) return;
      setAskedN((n) => n + 1);
      if (g.error || !g.text) { setPlugNote(`The gallery entry did not come: ${g.error ?? "empty"}`); return; }
      widgets.importText(g.text).then((id) => arrived(id, "the gallery")).catch((e) => setPlugNote(`Not added: ${e}`));
    });
    refresh();
    core.mcp().catch(() => {});
    core.hook().catch(() => {});
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") close.current(); };
    window.addEventListener("keydown", esc);
    return () => { for (const o of [off, offMcp, offStatus, offHook, offGallery, offItem]) o.then((f) => f()); window.removeEventListener("keydown", esc); };
  }, []);
  const save = async (name: string) => {
    await core.login(name, keys[name] ?? "");
    setKeys((k) => ({ ...k, [name]: "" }));
    refresh(); core.models().catch(() => {});
  };
  const label = "mb-2 mt-5 text-[11.5px] font-medium text-faint first:mt-0";
  return (
    <div onMouseDown={onClose} className="fixed inset-0 z-30 grid place-items-center bg-ink/15 p-10">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Settings" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className="flex max-h-full w-[640px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
        <div className="flex items-center border-b border-line-soft px-5 py-3">
          <span className="flex-1 font-medium">Settings</span>
          <button onClick={onClose} title="Close (Esc)" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="overflow-auto px-5 py-4">
          {!st && <div className="text-[12.5px] text-faint">Loading…</div>}
          {st && <>
            <div className={label}>Providers</div>
            <div className="mb-2 text-[12px] text-muted">Claude runs on your subscription through the claude CLI (its own login). Other providers need an API key; tazuna saves it on this machine, readable by you only, and never shows it again.</div>
            {st.providers.map((p) => (
              <div key={p.name} className="flex items-center gap-2 border-b border-line-soft py-2 last:border-b-0">
                <span className="w-24 text-ink">{p.name}</span>
                <span className={`w-36 text-[12px] ${p.key ? "text-ok" : "text-faint"}`}>{p.key || "no key"}</span>
                <input type="password" value={keys[p.name] ?? ""} onChange={(e) => setKeys((k) => ({ ...k, [p.name]: e.target.value }))}
                  onKeyDown={(e) => { if (e.key === "Enter" && keys[p.name]) save(p.name); }}
                  placeholder={p.key ? "replace the key" : p.example ? `key (${p.example})` : "key"} autoComplete="off" spellCheck={false}
                  className="min-w-0 flex-1 rounded-md border border-line bg-field px-2 py-1 font-mono text-[12px] text-ink outline-none placeholder:font-sans placeholder:text-faint" />
                <button disabled={!keys[p.name]} onClick={() => save(p.name)} className="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:bg-hover disabled:opacity-40">Save</button>
                <button onClick={() => core.openKeyPage(p.page)} title={`Make a key in ${p.place}`} className="rounded p-1 text-muted hover:bg-hover"><ExternalLink size={14} strokeWidth={1.5} /></button>
              </div>
            ))}

            <div className={label}>Who may see your code (this session)</div>
            <label className="flex items-start gap-2 py-1"><input type="radio" checked={!st.data_any} onChange={() => { setAskAny(false); core.setData("trusted").then(refresh); }} className="mt-1" />
              <span><span className="text-ink">Trusted providers only</span><span className="block text-[12px] text-muted">The providers you saved a key for, and the ones on your trusted list.</span></span></label>
            <label className="flex items-start gap-2 py-1"><input type="radio" checked={st.data_any} onChange={() => setAskAny(true)} className="mt-1" />
              <span><span className="text-ink">Any provider</span><span className="block text-[12px] text-muted">The model picker may use providers off the trusted list, which may store your code or train on it.</span></span></label>
            {askAny && !st.data_any && (
              <div className="ml-6 mt-1 flex items-center gap-2 rounded-md border border-line-strong px-3 py-2 text-[12px] text-ink-2">
                <span className="flex-1 text-warn">For a work repository this may not be allowed.</span>
                <button onClick={() => core.setData("any", true).then(() => { setAskAny(false); refresh(); })} className="rounded-md border border-line px-2.5 py-1 text-ink hover:bg-hover">Allow for this session</button>
                <button onClick={() => setAskAny(false)} className="rounded-md px-2 py-1 text-muted hover:bg-hover">Cancel</button>
              </div>
            )}

            <div className={label}>Chat or agent</div>
            <label className="flex items-start gap-2 py-1"><input type="checkbox" checked={st.route} onChange={(e) => core.setRoute(e.target.checked).then(refresh)} className="mt-1" />
              <span><span className="text-ink">Answer general questions inside a project as a chat</span><span className="block text-[12px] text-muted">Off: everything you send in a project goes to the agent.</span></span></label>

            <div className={label}>Paid API spend</div>
            <div className="mb-1.5 text-[12px] text-muted">Models on a provider's API cost money (Claude on your plan does not). tazuna sends nothing to them once the last 24 hours reach this cap.
              Last 24 hours: <span className="font-mono text-ink">${(st.spent_24h ?? 0).toFixed(2)}</span>.</div>
            <div className="flex items-center gap-2 py-1">
              <span className="text-ink-2">$</span>
              <input value={cap ?? (st.spend_cap == null ? "" : String(st.spend_cap))} onChange={(e) => setCap(e.target.value)} placeholder="no cap" inputMode="decimal"
                className="w-24 rounded-md border border-line bg-field px-2 py-1 font-mono text-[12px] text-ink outline-none" />
              <span className="text-[12px] text-muted">per 24 hours</span>
              <button disabled={cap == null || (cap.trim() !== "" && !(Number(cap) >= 0))} onClick={() => core.setSpendCap(cap!.trim() === "" ? null : Number(cap)).then(() => { setCap(null); refresh(); })}
                className="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:bg-hover disabled:opacity-40">Save</button>
              <span className="text-[11.5px] text-faint">empty: no cap</span>
            </div>

            <div className={label}>Tools from MCP servers</div>
            <div className="mb-1.5 text-[12px] text-muted">Servers for every model, from tazuna's list (the terminal's <code>/mcp</code>).
              A server runs its command on this machine: add only ones you trust.</div>
            {mcp && <>
              <label className="flex items-start gap-2 py-1"><input type="checkbox" checked={mcp.on} onChange={(e) => core.mcpOn(e.target.checked)} className="mt-1" />
                <span><span className="text-ink">Use them in this conversation</span><span className="block text-[12px] text-muted">
                  Remembered for new conversations.{mcp.claude ? " On Claude, your claude.ai connectors come too." : ""} Their tools take room in every request.</span></span></label>
              {mcp.note && <div className="py-1 text-[12px] text-bad">{mcp.note}</div>}
              {mcp.servers.length === 0 && <div className="py-1 text-[12.5px] text-faint">None yet.</div>}
              {mcp.servers.map((m) => {
                const st = mcpState[m.name];
                return (
                  <div key={m.name} className="flex items-center gap-2 border-b border-line-soft py-2 last:border-b-0">
                    <span className="min-w-0 flex-1"><span className="block text-ink">{m.name}</span>
                      <span className="block truncate font-mono text-[11.5px] text-faint" title={m.target}>{m.target}</span>
                      {st && <span role="status" className={`block text-[11.5px] ${st.ok === false ? "text-bad" : st.ok ? "text-ok" : "text-muted"}`}>{st.text}</span>}</span>
                    <button onClick={() => { setMcpState((all) => ({ ...all, [m.name]: { text: "connecting…" } })); core.mcpCheck(m.name); }}
                      className="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink-2 hover:bg-hover">Check</button>
                    {/^https?:/.test(m.target) && <button onClick={() => { setMcpState((all) => ({ ...all, [m.name]: { text: "logging in…" } })); core.mcpCheck(m.name, true); }}
                      title="Log in again in the browser" className="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink-2 hover:bg-hover">Log in</button>}
                    <button onClick={() => core.mcpRemove(m.name)} aria-label={`Remove ${m.name}`} title="Remove" className="rounded p-1 text-muted hover:bg-hover"><X size={14} strokeWidth={1.5} /></button>
                  </div>
                );
              })}
              <div className="flex items-center gap-2 py-2">
                <input value={adding.name} onChange={(e) => setAdding({ ...adding, name: e.target.value })} placeholder="name" aria-label="Server name" spellCheck={false}
                  className="w-28 rounded-md border border-line bg-field px-2 py-1 font-mono text-[12px] text-ink outline-none placeholder:font-sans placeholder:text-faint" />
                <input value={adding.target} onChange={(e) => setAdding({ ...adding, target: e.target.value })} placeholder="https://… or a command and its arguments"
                  aria-label="Server URL or command" spellCheck={false} onKeyDown={(e) => { if (e.key === "Enter" && canAdd) addServer(); }}
                  className="min-w-0 flex-1 rounded-md border border-line bg-field px-2 py-1 font-mono text-[12px] text-ink outline-none placeholder:font-sans placeholder:text-faint" />
                <button disabled={!canAdd} onClick={addServer} className="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:bg-hover disabled:opacity-40">Add</button>
              </div>
            </>}

            {hook && <>
              <div className={label}>After each turn that changed files</div>
              <div className="mb-1.5 text-[12px] text-muted">A command of yours for <span className="font-mono">{hook.cwd}</span>, such as a formatter. It runs there at no
                token cost: the model never sees it, and only a failure shows. Kept in your settings, never the repository's (the terminal's <code>/hook</code>).</div>
              <div className="flex items-center gap-2 py-1">
                <input value={hookCmd ?? hook.after_turn ?? ""} onChange={(e) => setHookCmd(e.target.value)} placeholder="none, e.g. cargo fmt" aria-label="Command after each turn"
                  spellCheck={false} onKeyDown={(e) => { if (e.key === "Enter" && hookCmd != null) core.hook(hookCmd.trim()).then(() => setHookCmd(null)); }}
                  className="min-w-0 flex-1 rounded-md border border-line bg-field px-2 py-1 font-mono text-[12px] text-ink outline-none placeholder:font-sans placeholder:text-faint" />
                <button disabled={hookCmd == null} onClick={() => core.hook(hookCmd!.trim()).then(() => setHookCmd(null))}
                  className="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:bg-hover disabled:opacity-40">Save</button>
              </div>
              {hook.note && <div className="py-1 text-[12px] text-bad">{hook.note}</div>}
            </>}

            <div className={label}>Quick chat from anywhere</div>
            <div className="flex items-center gap-2 py-1">
              <input value={keysNow} onChange={(e) => setKeysNow(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") saveHotkey(); }}
                placeholder="none" spellCheck={false} className="w-44 rounded-md border border-line bg-field px-2 py-1 font-mono text-[12px] text-ink outline-none" />
              <button onClick={saveHotkey} className="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:bg-hover">Save</button>
              <span className="text-[12px] text-muted">{hotkeyNote || "brings tazuna up with a new chat. Like Alt+Space or CmdOrCtrl+Shift+K; empty: off."}</span>
            </div>

            <div className={label}>Panels and plugins</div>
            <div className="mb-2 text-[12px] text-muted">Panels are sandboxed: they only show the conversation's numbers. Plugins run inside tazuna's
              window with the same reach: they can change any part of it and send messages to the agent, which can act on your files.
              Enable only plugins you made or trust. <code>/create …</code> builds either.</div>
            {items.length === 0 && <div className="py-1 text-[12.5px] text-faint">None yet.</div>}
            {items.map((w) => (
              <div key={w.id} className="flex items-center gap-2 border-b border-line-soft py-2 last:border-b-0">
                <span className="min-w-0 flex-1"><span className="block truncate text-ink">{w.title}</span>
                  <span className="block truncate text-[11.5px] text-faint">{w.kind === "panel" ? "panel · sandboxed" : w.state === "enabled" ? "plugin · on"
                    : w.state === "changed" ? "plugin · its code changed since you enabled it: look before enabling again" : "plugin · off"}{w.about ? ` · ${w.about}` : ""}</span></span>
                {w.kind === "plugin" && <button onClick={() => (w.state === "enabled" ? onEnable?.(w, false) : read(w))}
                  className={`rounded-md border px-2.5 py-1 text-[12px] hover:bg-hover ${w.state === "enabled" ? "border-line text-ink-2" : "border-line-strong text-ink"}`}>{w.state === "enabled" ? "Turn off" : "Read and enable"}</button>}
                <button onClick={() => widgets.share(w.id).then((p) => p && setPlugNote(`Saved ${w.title} to ${p}: anyone can add it with "Add from a file".`)).catch((e) => setPlugNote(`Not saved: ${e}`))}
                  title="Save it as one file to give to someone" className="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink-2 hover:bg-hover">Share</button>
                <button onClick={() => { onClose(); onEdit?.(w.id); }} className="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink-2 hover:bg-hover">Change with tazuna</button>
                <button onClick={() => onRemove?.(w.id)} title="Remove (its folder moves to .removed)" className="rounded p-1 text-muted hover:bg-hover"><X size={14} strokeWidth={1.5} /></button>
              </div>
            ))}
            {reading && (
              <div role="region" aria-label={`${reading.w.title}: its code`} className="mt-2 rounded-md border border-line-strong px-3 py-2.5">
                <div className="text-[12.5px] text-ink">{reading.w.title}</div>
                {reading.w.imported && <div className="text-[12.5px] font-medium text-warn">It came from someone else (a shared file or the gallery). Enable it only if you
                  have read the code and trust where it came from.</div>}
                <div className="mb-1.5 text-[12px] text-muted">{reading.would}. A plugin can act differently later: the code below is what counts. It runs inside the
                  window with tazuna's reach (it can send messages to the agent, which can act on your files).</div>
                <pre tabIndex={0} className="max-h-56 overflow-auto whitespace-pre-wrap break-all rounded-md bg-field px-2.5 py-2 font-mono text-[11.5px] text-ink select-text">{reading.code}</pre>
                <div className="mt-2 flex justify-end gap-2">
                  <button onClick={() => setReading(null)} className="rounded-md px-2.5 py-1 text-[12px] text-muted hover:bg-hover">Cancel</button>
                  <button onClick={() => { onEnable?.({ ...reading.w, print: reading.print }, true); setReading(null); }}
                    className="rounded-md bg-accent px-2.5 py-1 text-[12px] font-medium text-on-accent">Enable this code</button>
                </div>
              </div>
            )}
            <div className="mt-2 flex items-center gap-2">
              <button onClick={() => widgets.importFile().then((id) => arrived(id, "the file")).catch((e) => setPlugNote(`Not added: ${e}`))}
                className="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:bg-hover">Add from a file…</button>
              <button onClick={() => { setGallery({}); core.gallery().catch(() => {}); }} className="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:bg-hover">Browse the gallery</button>
            </div>
            {plugNote && <div role="status" className="mt-1.5 text-[12px] text-muted">{plugNote}</div>}
            {gallery && (
              <div className="mt-2 rounded-md border border-line px-3 py-1">
                {!gallery.items && !gallery.error && <div className="py-1.5 text-[12.5px] text-faint">Loading the gallery…</div>}
                {gallery.error && <div className="py-1.5 text-[12.5px] text-bad">{gallery.error}</div>}
                {gallery.items?.map((g) => (
                  <div key={g.file} className="flex items-center gap-2 border-b border-line-soft py-2 last:border-b-0">
                    <span className="min-w-0 flex-1"><span className="block text-ink">{g.title} <span className="text-[11.5px] text-faint">· {g.kind}{g.author ? ` · by ${g.author}` : ""}</span></span>
                      <span className="block text-[12px] text-muted">{g.about}</span></span>
                    <button disabled={asked.current.has(g.file)} onClick={() => { asked.current.add(g.file); setAskedN((n) => n + 1); core.galleryGet(g.file); }}
                      className="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:bg-hover disabled:opacity-40">{asked.current.has(g.file) ? "Adding…" : "Add"}</button>
                  </div>
                ))}
                <div className="py-1.5 text-[11.5px] text-faint">From gallery/ in tazuna's repository; anyone can propose one there with a pull request.</div>
              </div>
            )}
            <label className="mt-3 flex items-start gap-2 py-1"><input type="checkbox" checked={safeMode()} onChange={(e) => {
              if (e.target.checked) localStorage.setItem("tazuna.safe", "1"); else localStorage.removeItem("tazuna.safe");
              location.reload();
            }} className="mt-1" />
              <span><span className="text-ink">Safe mode: start without plugins</span><span className="block text-[12px] text-muted">For when a plugin breaks the window. Panels stay.</span></span></label>

            <div className={label}>Updates</div>
            <div className="flex items-center gap-2 py-1">
              <span className="flex-1 text-[12.5px] text-ink-2" title="The app carries its own tazuna core, separate from the terminal's">
                tazuna desktop {appVersion || "?"} <span className="text-faint">· its core {st.version}</span></span>
              {update.version
                ? update.installing ? <span role="status" className="text-[12px] text-muted">Installing… tazuna restarts by itself.</span>
                  : <button onClick={() => installUpdate()} title="Replies running now stop; conversations stay resumable"
                      className="rounded-md bg-accent px-2.5 py-1 text-[12px] font-medium text-on-accent">Install {update.version} and restart</button>
                : <button disabled={checking} onClick={async () => { setChecking(true); await checkUpdate(true); setChecking(false); }}
                    className="rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:bg-hover disabled:opacity-60">{checking ? "Checking…" : "Check for updates"}</button>}
            </div>
            {update.version && !update.installing && <div className="text-[12px] text-muted">tazuna desktop {update.version} is out. Replies running now stop when it restarts.</div>}
            {update.upToDate && <div role="status" className="text-[12px] text-ok">Up to date.</div>}
            {update.error && <div role="alert" className="text-[12px] text-bad">{update.error}</div>}
          </>}
        </div>
      </div>
    </div>
  );
}
