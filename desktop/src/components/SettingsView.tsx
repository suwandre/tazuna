import { ExternalLink, RefreshCw, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useDialog } from "../lib/dialog";
import { checkUpdate, installUpdate, progressText, useUpdate } from "../lib/update";
import { safeMode } from "../lib/plugins";
import { HOTKEY, core, defaultHotkey, type GalleryItem, type Hook, type Mcp, type Settings } from "../lib/tazuna";
import { dryRun, summary, widgets, type UserWidget } from "../lib/widgets";

// Settings: provider keys (saved by tazuna, never shown again), the data policy for this session, the router. The
// terminal's /login, /data and /route in one page. Each part is a section: a heading, a line on what it is for, and a card
// of rows (a setting's name and what it does on the left, its control on the right), so the page reads as parts and
// not as one run of text (2026-11-18).
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
  const input = "rounded-md border border-line bg-field px-2 py-1 font-mono text-[12px] text-ink outline-none focus:border-accent placeholder:font-sans placeholder:text-faint";
  const button = "rounded-md border border-line px-2.5 py-1 text-[12px] text-ink disabled:opacity-40";
  return (
    <div onMouseDown={onClose} className="tz-fade fixed inset-0 z-30 grid place-items-center bg-scrim p-10">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Settings" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className="tz-dialog flex max-h-full w-[720px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
        <div className="flex items-center border-b border-line-soft px-5 py-3">
          <span className="flex-1 font-medium">Settings</span>
          <button onClick={onClose} title="Close (Esc)" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="overflow-auto bg-sidebar px-6 pb-7 pt-5">
          {!st && <div className="text-[12.5px] text-faint">Loading…</div>}
          {st && <>
            <Section title="Providers" note="Claude runs on your subscription through the claude CLI, with its own login. Other providers need an API key: tazuna saves it on this machine, readable by you only, and never shows it again.">
              {st.providers.map((p) => (
                <Row key={p.name} title={p.name} note={<span className={p.key ? "text-ok" : ""}>{p.key || "no key"}</span>}>
                  <input type="password" value={keys[p.name] ?? ""} onChange={(e) => setKeys((k) => ({ ...k, [p.name]: e.target.value }))}
                    onKeyDown={(e) => { if (e.key === "Enter" && keys[p.name]) save(p.name); }} aria-label={`${p.name} key`}
                    placeholder={p.key ? "replace the key" : p.example ? `key (${p.example})` : "key"} autoComplete="off" spellCheck={false} className={`w-56 ${input}`} />
                  <button disabled={!keys[p.name]} onClick={() => save(p.name)} className={button}>Save</button>
                  <button onClick={() => core.openKeyPage(p.page)} aria-label={`Make a key in ${p.place}`} title={`Make a key in ${p.place}`} className="rounded p-1 text-muted hover:text-ink"><ExternalLink size={14} strokeWidth={1.5} /></button>
                </Row>
              ))}
            </Section>

            <Section title="Privacy and routing" note="Where your code may go in this session, and what answers a question.">
              <Row title="Who may see your code" note={st.data_any ? "Any provider, for this session: the picker may use providers off the trusted list, which may store your code or train on it."
                : "Trusted providers only: the ones you saved a key for, and the ones on your trusted list."}>
                <Segmented label="Who may see your code" value={st.data_any || askAny ? "any" : "trusted"} options={[["trusted", "Trusted only"], ["any", "Any provider"]]}
                  onChange={(v) => { if (v === "trusted") { setAskAny(false); core.setData("trusted").then(refresh); } else if (!st.data_any) setAskAny(true); }} />
              </Row>
              {askAny && !st.data_any && (
                <div className="flex items-center gap-2 px-4 py-2.5 text-[12px]">
                  <span className="flex-1 text-warn">For a work repository this may not be allowed.</span>
                  <button onClick={() => core.setData("any", true).then(() => { setAskAny(false); refresh(); })} className={button}>Allow for this session</button>
                  <button onClick={() => setAskAny(false)} className="rounded-md px-2 py-1 text-muted">Cancel</button>
                </div>
              )}
              <Row title="Answer general questions as a chat" note="Inside a project, a question that is not about the code gets a chat answer. Off: everything you send goes to the agent.">
                <Switch label="Answer general questions as a chat" on={st.route} onChange={(on) => core.setRoute(on).then(refresh)} />
              </Row>
            </Section>

            <Section title="Paid API spend" note="Models on a provider's API cost money; Claude on your plan does not.">
              <Row title="Cap per 24 hours" note={<>tazuna sends nothing to paid APIs once the last 24 hours reach it. Last 24 hours: <span className="font-medium tabular-nums text-ink">${(st.spent_24h ?? 0).toFixed(2)}</span>. Empty: no cap.</>}>
                <span className="text-ink-2">$</span>
                <input value={cap ?? (st.spend_cap == null ? "" : String(st.spend_cap))} onChange={(e) => setCap(e.target.value)} placeholder="no cap" inputMode="decimal" aria-label="Cap in dollars per 24 hours" className={`w-24 ${input}`} />
                <button disabled={cap == null || (cap.trim() !== "" && !(Number(cap) >= 0))} onClick={() => core.setSpendCap(cap!.trim() === "" ? null : Number(cap)).then(() => { setCap(null); refresh(); })} className={button}>Save</button>
              </Row>
            </Section>

            <Section title="Tools from MCP servers" note={<>Servers for every model, from tazuna's list (the terminal's <code>/mcp</code>). A server runs its command on this machine: add only ones you trust.</>}>
              {mcp ? <>
                <Row title="Use them in this conversation" note={`Remembered for new conversations.${mcp.claude ? " On Claude, your claude.ai connectors come too." : ""} Their tools take room in every request.`}>
                  <Switch label="Use MCP servers in this conversation" on={mcp.on} onChange={(on) => core.mcpOn(on)} />
                </Row>
                {mcp.note && <div className="px-4 py-2.5 text-[12px] text-bad">{mcp.note}</div>}
                {mcp.servers.length === 0 && <div className="px-4 py-2.5 text-[12.5px] text-faint">No servers yet.</div>}
                {mcp.servers.map((m) => {
                  const ms = mcpState[m.name];
                  return (
                    <Row key={m.name} title={m.name} note={<><span className="block truncate font-mono text-[11.5px] text-faint" title={m.target}>{m.target}</span>
                      {ms && <span role="status" className={`block text-[11.5px] ${ms.ok === false ? "text-bad" : ms.ok ? "text-ok" : "text-muted"}`}>{ms.text}</span>}</>}>
                      <button onClick={() => { setMcpState((all) => ({ ...all, [m.name]: { text: "connecting…" } })); core.mcpCheck(m.name); }} className={button}>Check</button>
                      {/^https?:/.test(m.target) && <button onClick={() => { setMcpState((all) => ({ ...all, [m.name]: { text: "logging in…" } })); core.mcpCheck(m.name, true); }}
                        title="Log in again in the browser" className={button}>Log in</button>}
                      <button onClick={() => core.mcpRemove(m.name)} aria-label={`Remove ${m.name}`} title="Remove" className="rounded p-1 text-muted hover:text-ink"><X size={14} strokeWidth={1.5} /></button>
                    </Row>
                  );
                })}
                <div className="flex items-center gap-2 px-4 py-3">
                  <input value={adding.name} onChange={(e) => setAdding({ ...adding, name: e.target.value })} placeholder="name" aria-label="Server name" spellCheck={false} className={`w-28 ${input}`} />
                  <input value={adding.target} onChange={(e) => setAdding({ ...adding, target: e.target.value })} placeholder="https://… or a command and its arguments"
                    aria-label="Server URL or command" spellCheck={false} onKeyDown={(e) => { if (e.key === "Enter" && canAdd) addServer(); }} className={`min-w-0 flex-1 ${input}`} />
                  <button disabled={!canAdd} onClick={addServer} className={button}>Add</button>
                </div>
              </> : <div className="px-4 py-2.5 text-[12.5px] text-faint">Loading…</div>}
            </Section>

            {hook && (
              <Section title="After each turn that changed files" note={<>A command of yours for <span className="font-mono">{hook.cwd}</span>, such as a formatter. It runs there at no token cost: the model
                never sees it, and only a failure shows. Kept in your settings, never the repository's (the terminal's <code>/hook</code>).</>}>
                <div className="flex items-center gap-2 px-4 py-3">
                  <input value={hookCmd ?? hook.after_turn ?? ""} onChange={(e) => setHookCmd(e.target.value)} placeholder="none, e.g. cargo fmt" aria-label="Command after each turn"
                    spellCheck={false} onKeyDown={(e) => { if (e.key === "Enter" && hookCmd != null) core.hook(hookCmd.trim()).then(() => setHookCmd(null)); }} className={`min-w-0 flex-1 ${input}`} />
                  <button disabled={hookCmd == null} onClick={() => core.hook(hookCmd!.trim()).then(() => setHookCmd(null))} className={button}>Save</button>
                </div>
                {hook.note && <div className="px-4 py-2.5 text-[12px] text-bad">{hook.note}</div>}
              </Section>
            )}

            <Section title="Quick chat from anywhere">
              <Row title="Shortcut" note={hotkeyNote || "Brings tazuna up with a new chat, from any app: for example Alt+Space or Cmd+Shift+K. Empty: off."}>
                <input value={keysNow} onChange={(e) => setKeysNow(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") saveHotkey(); }} aria-label="Shortcut"
                  placeholder="none" spellCheck={false} className={`w-40 ${input}`} />
                <button onClick={saveHotkey} className={button}>Save</button>
              </Row>
            </Section>

            <Section title="Panels and plugins" note={<>A panel only shows the conversation's numbers. A plugin can change any part of the window and send the agent
              messages, so enable only plugins you made or trust. <code>/create</code> in a conversation makes either.</>}>
              {items.length === 0 && <div className="px-4 py-2.5 text-[12.5px] text-faint">No panels or plugins yet.</div>}
              {items.map((w) => (
                <Row key={w.id} title={w.title} note={<span className="block truncate">{w.kind === "panel" ? "panel · sandboxed" : w.state === "enabled" ? "plugin · on"
                  : w.state === "changed" ? "plugin · its code changed since you enabled it: look before enabling again" : "plugin · off"}{w.about ? ` · ${w.about}` : ""}</span>}>
                  {w.kind === "plugin" && <button onClick={() => (w.state === "enabled" ? onEnable?.(w, false) : read(w))}
                    className={`${button} ${w.state === "enabled" ? "text-ink-2" : "border-line-strong"}`}>{w.state === "enabled" ? "Turn off" : "Read and enable"}</button>}
                  <button onClick={() => widgets.share(w.id).then((p) => p && setPlugNote(`Saved ${w.title} to ${p}: anyone can add it with "Add from a file".`)).catch((e) => setPlugNote(`Not saved: ${e}`))}
                    title="Save it as one file to give to someone" className={button}>Share</button>
                  <button onClick={() => { onClose(); onEdit?.(w.id); }} className={button}>Change with tazuna</button>
                  <button onClick={() => onRemove?.(w.id)} aria-label={`Remove ${w.title}`} title="Remove (its folder moves to .removed)" className="rounded p-1 text-muted hover:text-ink"><X size={14} strokeWidth={1.5} /></button>
                </Row>
              ))}
              {reading && (
                <div role="region" aria-label={`${reading.w.title}: its code`} className="px-4 py-3">
                  <div className="text-[12.5px] font-medium text-ink">{reading.w.title}</div>
                  {reading.w.imported && <div className="text-[12.5px] font-medium text-warn">It came from someone else (a shared file or the gallery). Enable it only if you
                    have read the code and trust where it came from.</div>}
                  <div className="mb-1.5 text-[12px] text-muted">{reading.would}. A plugin can act differently later: the code below is what counts. It runs inside the
                    window with tazuna's reach (it can send messages to the agent, which can act on your files).</div>
                  <pre tabIndex={0} className="max-h-56 overflow-auto whitespace-pre-wrap break-all rounded-md bg-field px-2.5 py-2 font-mono text-[11.5px] text-ink select-text">{reading.code}</pre>
                  <div className="mt-2 flex justify-end gap-2">
                    <button onClick={() => setReading(null)} className="rounded-md px-2.5 py-1 text-[12px] text-muted">Cancel</button>
                    <button onClick={() => { onEnable?.({ ...reading.w, print: reading.print }, true); setReading(null); }}
                      className="rounded-md bg-accent px-2.5 py-1 text-[12px] font-medium text-on-accent">Enable this code</button>
                  </div>
                </div>
              )}
              <div className="px-4 py-3">
                <div className="flex items-center gap-2">
                  <button onClick={() => widgets.importFile().then((id) => arrived(id, "the file")).catch((e) => setPlugNote(`Not added: ${e}`))} className={button}>Add from a file…</button>
                  <button onClick={() => { setGallery({}); core.gallery().catch(() => {}); }} className={button}>Browse the gallery</button>
                </div>
                {plugNote && <div role="status" className="mt-1.5 text-[12px] text-muted">{plugNote}</div>}
              </div>
              {gallery && <>
                {!gallery.items && !gallery.error && <div className="px-4 py-2.5 text-[12.5px] text-faint">Loading the gallery…</div>}
                {gallery.error && <div className="px-4 py-2.5 text-[12.5px] text-bad">{gallery.error}</div>}
                {gallery.items?.map((g) => (
                  <Row key={g.file} title={<>{g.title} <span className="text-[11.5px] font-normal text-faint">· {g.kind}{g.author ? ` · by ${g.author}` : ""}</span></>} note={g.about}>
                    <button disabled={asked.current.has(g.file)} onClick={() => { asked.current.add(g.file); setAskedN((n) => n + 1); core.galleryGet(g.file); }}
                      className={button}>{asked.current.has(g.file) ? "Adding…" : "Add"}</button>
                  </Row>
                ))}
                {gallery.items && <div className="px-4 py-2.5 text-[11.5px] text-faint">From gallery/ in tazuna's repository; anyone can propose one there with a pull request.</div>}
              </>}
              <Row title="Safe mode: start without plugins" note="For when a plugin breaks the window. Panels stay. The window reloads.">
                <Switch label="Safe mode" on={safeMode()} onChange={(on) => { if (on) localStorage.setItem("tazuna.safe", "1"); else localStorage.removeItem("tazuna.safe"); location.reload(); }} />
              </Row>
            </Section>

            <Section title="About">
              <Row title={<span className="flex items-center gap-2">Version <span className="rounded border border-line bg-field px-1.5 py-px font-mono text-[11.5px] font-normal text-ink-2">{appVersion || "?"}</span></span>}
                note={<>tazuna desktop, with its own core (<span className="font-mono">{st.version}</span>), separate from the terminal's. It looks for updates every hour.</>}>
                {update.version
                  ? update.installing ? <span role="status" className="text-[12px] text-muted">{progressText(update)}</span>
                    : <button onClick={() => installUpdate()} title="Replies running now stop; conversations stay resumable"
                        className="rounded-md bg-accent px-2.5 py-1 text-[12px] font-medium text-on-accent">Install {update.version} and restart</button>
                  // aria-disabled, not disabled, while checking: a disabled button drops focus, which then had nowhere to
                  // come back to when the answer's dialog closed
                  : <button aria-disabled={checking} onClick={async () => { if (checking) return; setChecking(true); await checkUpdate(true); setChecking(false); }}
                      className={`flex items-center gap-1.5 ${button} aria-disabled:opacity-60`}><RefreshCw size={12} strokeWidth={1.75} className={checking ? "animate-spin" : ""} aria-hidden="true" />{checking ? "Checking…" : "Check for updates"}</button>}
              </Row>
            </Section>
          </>}
        </div>
      </div>
    </div>
  );
}

/** A part of the page: its heading, a line on what it is for, and its rows in one card, a line between each. */
function Section({ title, note, children }: { title: string; note?: React.ReactNode; children: React.ReactNode }) {
  return (
    <section className="mt-7 first:mt-0">
      <h3 className="text-[13px] font-semibold text-ink">{title}</h3>
      {note && <p className="mt-0.5 text-[12px] leading-relaxed text-muted">{note}</p>}
      <div className="mt-2.5 divide-y divide-line-soft rounded-lg border border-line bg-card">{children}</div>
    </section>
  );
}

/** One setting: its name and what it does on the left, its control on the right. */
function Row({ title, note, children }: { title: React.ReactNode; note?: React.ReactNode; children?: React.ReactNode }) {
  return (
    <div className="flex items-center gap-4 px-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="text-[13px] font-medium text-ink">{title}</div>
        {note && <div className="mt-0.5 text-[12px] leading-relaxed text-muted">{note}</div>}
      </div>
      {children && <div className="flex flex-none items-center gap-2">{children}</div>}
    </div>
  );
}

/** On or off. */
function Switch({ on, onChange, label }: { on: boolean; onChange: (on: boolean) => void; label: string }) {
  return (
    <button role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)}
      className={`relative h-5 w-9 flex-none rounded-full ${on ? "bg-accent" : "bg-line-strong"}`}>
      <span className={`absolute left-0.5 top-0.5 size-4 rounded-full bg-white shadow-sm transition-transform duration-150 ${on ? "translate-x-4" : ""}`} />
    </button>
  );
}

/** One of a few, side by side. */
function Segmented({ value, options, onChange, label }: { value: string; options: [string, string][]; onChange: (v: string) => void; label: string }) {
  return (
    <div role="radiogroup" aria-label={label} className="flex rounded-md border border-line p-0.5 text-[12px]">
      {options.map(([v, l]) => (
        <button key={v} role="radio" aria-checked={v === value} onClick={() => onChange(v)}
          className={`whitespace-nowrap rounded px-2.5 py-1 ${v === value ? "bg-selected text-ink" : "text-muted hover:text-ink"}`}>{l}</button>
      ))}
    </div>
  );
}
