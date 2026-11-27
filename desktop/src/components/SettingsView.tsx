import { Check, ChevronRight, Copy, ExternalLink, LoaderCircle, RefreshCw, Search, X } from "lucide-react";
import { createContext, useContext, useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useDialog } from "../lib/dialog";
import { checkUpdate, installUpdate, progressText, useUpdate } from "../lib/update";
import { safeMode } from "../lib/plugins";
import { HOTKEY, core, defaultHotkey, usageReq, type Allowance, type GalleryItem, type Hook, type Learned, type Mcp, type Provider, type Settings, type SignIn, type UsageMethod, type UsageSource } from "../lib/tazuna";
import { asked0, askUsage, cap as capital, usageAnswer, usageExpire, usageState, type Asked } from "../lib/health";
import { matchesWords } from "../lib/types";
import { dryRun, summary, widgets, type UserWidget } from "../lib/widgets";
import { ACCENT, ACCENTS, LIVE, NUMBERS, PROSE, STATS, TEXT_SIZES, THEME, ZOOM, accentNow, look } from "../lib/chrome";
import { Parts } from "./Sep";
import { accentButton, Segmented, Swatches, dialogShell, quietButton } from "./Controls";
import { ClaudeButtons, ClaudeNote, useClaude } from "./Setup";

// Settings: provider keys (saved by tazuna, never shown again), the data policy for this session, the router. The
// terminal's /login, /data and /route in one page. Each part is a section: a heading, a line on what it is for, and a card
// of rows (a setting's name and what it does on the left, its control on the right), so the page reads as parts and
// not as one run of text (2026-11-18).
type PluginProps = {
  items?: UserWidget[]; onEnable?: (w: UserWidget, on: boolean) => void; onEdit?: (id: string) => void; onRemove?: (id: string) => void;
  /** a panel or plugin came in (a file, the gallery): the list again */
  onChanged?: () => void;
};

// The page's parts, one on screen at a time, chosen in the list on its left (until desktop 0.1.87 one scroll of eight parts
// in a 720 px dialog, with nothing to say where you were; design review 8). The last one open comes back.
export const TAB = "tazuna.settings.tab";
const TabCtx = createContext("");

export function SettingsView({ onClose, focus, items = [], onEnable, onEdit, onRemove, onChanged, chat }: { onClose: () => void;
  /** opened from a plain chat: what is a project's alone (the command after each turn) says so, with no home-folder command */
  chat?: boolean;
  /** opened on this provider (the model picker's "No key yet"): its row in view, its key field open */
  focus?: string | null } & PluginProps) {
  const [st, setSt] = useState<Settings | null>(null);
  // Claude Code: the first of your providers, signed in or how to sign in (core C1; Settings had no row for it)
  const [claude, checkClaude, checkingClaude] = useClaude();
  const [tab, setTabNow] = useState(() => localStorage.getItem(TAB) || "Appearance");
  const setTab = (t: string) => { localStorage.setItem(TAB, t); setTabNow(t); };
  // how the window looks (lib/chrome.ts keeps and applies it)
  const [looks, setLooks] = useState(() => ({ theme: localStorage.getItem(THEME) ?? "system", accent: accentNow(), zoom: localStorage.getItem(ZOOM) ?? "1", prose: localStorage.getItem(PROSE) ?? "serif",
    stats: localStorage.getItem(STATS) === "rows" ? "rows" : "tiles", numbers: localStorage.getItem(NUMBERS) ?? "under", live: localStorage.getItem(LIVE) === "off" ? "off" : "on" }));
  const setLook = (key: string, name: keyof typeof looks, v: string) => { look(key, v); setLooks((l) => ({ ...l, [name]: v })); };
  // a provider's key field shows when asked for (seven open fields down the page read as a form to fill in)
  const [keyFor, setKeyFor] = useState<string | null>(focus ?? null);
  // the providers without a key (some seventy since core 0.1.196) folded under one line, searched by name or host
  const [more, setMore] = useState(!!focus && focus !== "Claude");
  const [find, setFind] = useState("");
  const dialog = useDialog();
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [askAny, setAskAny] = useState(false);
  const [keysNow, setKeysNow] = useState(() => localStorage.getItem(HOTKEY) ?? defaultHotkey);
  const [hotkeyNote, setHotkeyNote] = useState("");
  const [cap, setCap] = useState<string | null>(null);
  const [mcp, setMcp] = useState<Mcp | null>(null);
  const [mcpState, setMcpState] = useState<Record<string, { ok?: boolean; text: string }>>({});
  // sign-ins (core oauth.rs): the one whose warning is being confirmed, each one's progress and outcome, the ChatGPT plan's
  // welcome (once, after the first sign-in that uses it: OpenAI's guidelines)
  const [confirming, setConfirming] = useState<string | null>(null);
  const [signing, setSigning] = useState<Record<string, SignIn>>({});
  const [welcome, setWelcome] = useState(false);
  const [adding, setAdding] = useState({ name: "", target: "" });
  const [hook, setHook] = useState<Hook | null>(null);
  // each provider's ways to read its usage and the user's choice (core usage sources), what the console reader's visits
  // found, and a word on the last change (refused, signed out of a console)
  const [sources, setSources] = useState<Record<string, UsageSource>>({});
  const [learned, setLearned] = useState<Record<string, Learned | null>>({});
  const [usageNote, setUsageNote] = useState<{ provider: string; text: string; bad?: boolean } | null>(null);
  // the usage requests in flight (lib/health.ts askUsage), by the window's own number (`req`, said back on its reply): a
  // button waits until then, and a track request's choice is what the line shows until its own reply, so a reply to an
  // earlier click never puts the picker back (the user's recording: Console, then CLI, and it jumped back); let go after
  // 20 s or when the core exits
  const flight = useRef<Asked>(asked0);
  const [, setFlown] = useState(0);
  const fly = (a: Asked) => { flight.current = a; setFlown((n) => n + 1); };
  const pending = (provider: string, kind: string) => flight.current.pend.find((x) => x.provider === provider && x.kind === kind);
  const ask = (provider: string, kind: "track" | "check" | "save", send: (req: number) => Promise<unknown>, track?: { on: boolean; method: string }) => {
    const req = usageReq();
    setUsageNote(null);
    fly(askUsage(flight.current, { provider, kind, req, at: Date.now(), track }));
    setTimeout(() => fly(usageExpire(flight.current, Date.now())), 20_100);
    send(req).catch(() => fly({ ...flight.current, pend: flight.current.pend.filter((x) => x.req !== req) }));
  };
  const learn = (provider: string) => core.consoleLearned(provider).then((l) => setLearned((all) => ({ ...all, [provider]: l })), () => {});
  // the app's own version beside its core's: "tazuna 0.1.173" alone read as the terminal's
  const [appVersion, setAppVersion] = useState("");
  useEffect(() => { getVersion().then(setAppVersion).catch(() => {}); }, []);
  useEffect(() => {
    if (!st || !focus) return;
    const at = document.querySelector<HTMLElement>(`[data-key-for="${CSS.escape(focus)}"], [aria-label="${CSS.escape(focus)} key"], [data-sign-in="${CSS.escape(focus)}"]`);
    at?.scrollIntoView({ block: "center" });
    if (at?.tagName === "BUTTON") at.focus();
  }, [!!st, claude !== undefined]);
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
    const offSign = core.onSignIn((m) => {
      setSigning((all) => ({ ...all, [m.provider]: m }));
      if (m.ok != null) { core.settings().catch(() => {}); core.models().catch(() => {}); }
      if (m.first) setWelcome(true);
    });
    const offGallery = core.onGallery(setGallery), offItem = core.onGalleryItem((g) => {
      if (g.file && !asked.current.delete(g.file)) return;
      setAskedN((n) => n + 1);
      if (g.error || !g.text) { setPlugNote(`The gallery entry did not come: ${g.error ?? "empty"}`); return; }
      widgets.importText(g.text).then((id) => arrived(id, "the gallery")).catch((e) => setPlugNote(`Not added: ${e}`));
    });
    const offSources = core.onUsageSources((list, error, req) => {
      // a reply answers its request and every one before it; a refusal is said on its own request's row
      const { a, provider } = usageAnswer(flight.current, req);
      if (a !== flight.current) fly(a);
      if (error && provider) setUsageNote({ provider, text: error, bad: true });
      if (!list) return;
      setSources(Object.fromEntries(list.map((x) => [x.provider, x])));
      for (const x of list) if (x.methods.some((m) => m.id === "console")) learn(x.provider);
    });
    const offConsole = core.onConsoleClosed(learn), offExit = core.onExit(() => fly(asked0));
    core.usageSources().catch(() => {});
    refresh();
    core.mcp().catch(() => {});
    core.hook().catch(() => {});
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") close.current(); };
    window.addEventListener("keydown", esc);
    return () => { for (const o of [off, offMcp, offStatus, offHook, offGallery, offItem, offSign, offSources, offConsole, offExit]) o.then((f) => f()); window.removeEventListener("keydown", esc); };
  }, []);
  // `key` given: what to send whatever the field holds ("" for Mistral's "Yes": keep the key it has); how it went comes
  // back as a `signin` line for the row
  const save = async (name: string, key = keys[name] ?? "") => {
    setSigning((all) => { const { [name]: _, ...rest } = all; return rest; });
    await core.login(name, key);
    closeKey(name);
    refresh(); core.models().catch(() => {});
  };
  // a key field closed (Cancel, Escape, saved): what was typed in it goes, so nothing abandoned is sent later
  const closeKey = (name: string) => { setKeys((k) => ({ ...k, [name]: "" })); setKeyFor(null); };
  // a Gemini CLI account's Google Cloud project, typed on its row when its sign-in said it needs one
  const [projects, setProjects] = useState<Record<string, string>>({});
  const input = "rounded-md border border-line bg-field px-2 py-1 font-mono text-[12px] text-ink outline-none focus:border-accent placeholder:font-sans placeholder:text-faint";
  const button = quietButton;
  // Yours: the providers with a key and the model servers running here; the rest under More providers
  const yours = st?.providers.filter((p) => p.key || p.local) ?? [], others = st?.providers.filter((p) => !p.key && !p.local) ?? [];
  const found = others.filter((p) => matchesWords(find, `${p.name} ${p.host}`));
  const signIn = (p: Provider, other = false) => {
    setConfirming(null);
    setSigning((all) => ({ ...all, [p.name]: { provider: p.name, text: "Starting…" } }));
    (other ? core.signInOther(p.name) : core.signIn(p.name, p.warn, p.needs_project != null ? projects[p.name] ?? p.needs_project : undefined)).catch(() => {});
  };
  const busy = (p: Provider) => !!signing[p.name] && signing[p.name].ok == null;
  // tracked or not, and how: the choice shown at once and kept until its own reply (latest wins)
  const track = (u: UsageSource, on: boolean, method: string) => ask(u.provider, "track", (req) => core.usageTrack(u.provider, on, method, req), { on, method });
  const allow = (u: UsageSource, a: Allowance | null) => ask(u.provider, "save", (req) => core.usageAllowance(u.provider, a, req));
  const check = (u: UsageSource) => { if (!pending(u.provider, "check")) ask(u.provider, "check", (req) => core.usageSources(req, u.provider)); };
  // the usage line: on your providers' rows, on a console's without a key too (it reads with a sign-in), and on any other
  // row while it is tracked; a track request in flight shows its choice
  const usageOf = (p: Provider) => {
    const u = sources[p.name], t = pending(p.name, "track")?.track;
    return u && (p.key || p.local || u.track.on || u.methods.some((m) => m.id === "console")) ? (t ? { ...u, track: t } : u) : undefined;
  };
  const provider = (p: Provider) => (
    <Row key={p.name} title={p.name} below={usageOf(p) && <UsageLine u={usageOf(p)!} learned={learned[p.name]} track={track} allow={allow} check={check} refreshLearned={learn}
      busy={{ track: !!pending(p.name, "track"), check: !!pending(p.name, "check"), save: !!pending(p.name, "save") }}
      note={usageNote?.provider === p.name ? usageNote : null} say={(text, bad) => setUsageNote({ provider: p.name, text, bad })} />} note={<>{/* one line: with the key field open, "managed key (tazuna subrouter --manage)" broke inside "--manage" */}
      <span title={p.key || p.host} className={`block truncate ${p.plan_off || p.unsure ? "text-warn" : p.key ? "text-ok" : "font-mono text-[11.5px] text-faint"}`}>{p.key ? capital(p.key) : p.host}</span>
      {/* its terms, quiet; in the warning colour only where the account may be at risk or it may refuse tazuna */}
      {p.note && <span className={`block ${p.warn ? "text-warn" : "text-faint"}`}>{p.note}</span>}
      {/* a key the core cannot price for sure (Mistral's MISTRAL_API_KEY from the environment) */}
      {p.hint && <span className="block text-warn">{p.hint}</span>}
      {/* Mistral's key kept from before under Mistral Vibe: asked on both rows */}
      {p.unsure === "old_vibe" && p.name === "Mistral" && <span className="block text-warn">The key saved for both before stays under Mistral Vibe, unused: is it your Vibe key?</span>}
      {/* a sign-in: what to do now (a code to type, the browser tab), then how it went; a key's save, how it went */}
      {signing[p.name] && <span role="status" className={`block ${signing[p.name].ok === false ? "text-bad" : signing[p.name].ok ? "text-ok" : "text-ink-2"}`}>{signing[p.name].text}</span>}</>}>
      {/* the Google Cloud project a Gemini CLI account needs, before signing in again */}
      {p.needs_project != null && !p.key && <input value={projects[p.name] ?? p.needs_project} onChange={(e) => setProjects((all) => ({ ...all, [p.name]: e.target.value }))}
        aria-label={`${p.name} Google Cloud project`} placeholder="Google Cloud project ID" autoComplete="off" spellCheck={false} className={`w-44 ${input}`} />}
      {p.sign_in && <>
        {confirming === p.name ? <>
          <button onClick={() => setConfirming(null)} className={quietButton}>Cancel</button>
          <button autoFocus onClick={() => signIn(p)} className={accentButton}>I understand, sign in</button>
        </> : p.key ? <>
          {/* plan use declined at the consent page: a sign-in that asks for it again; ChatGPT: another account in its place */}
          {p.plan_off ? <button disabled={busy(p)} onClick={() => signIn(p)} className={button}>Allow plan use</button>
            : p.sign_in === "chatgpt" && <button disabled={busy(p)} onClick={() => signIn(p, true)} className={button}>Other account</button>}
          <button disabled={busy(p)} onClick={() => { setSigning((all) => ({ ...all, [p.name]: { provider: p.name, text: "Signing out…" } })); core.signOut(p.name).catch(() => {}); }}
            data-sign-in={p.name} className={button}>Sign out</button>
        </>
        // OpenAI's wording for its sign-in button; a warning to confirm first where the sign-in sends another app's identity
        : <button disabled={busy(p)} onClick={() => (p.warn && !p.confirmed ? setConfirming(p.name) : signIn(p))} data-sign-in={p.name}
          className={button}>{p.sign_in === "chatgpt" ? "Continue with ChatGPT" : "Sign in"}</button>}
        <button onClick={() => core.openKeyPage(p.page)} aria-label={`Open ${p.place}`} title={`Open ${p.place}`} className="rounded p-1 text-muted hover:text-ink"><ExternalLink size={14} strokeWidth={1.5} /></button>
      </>}
      {!p.local && !p.sign_in && <>
        {keyFor === p.name ? <>
          <input autoFocus type="password" value={keys[p.name] ?? ""} onChange={(e) => setKeys((k) => ({ ...k, [p.name]: e.target.value }))}
            onKeyDown={(e) => { if (e.key === "Enter" && keys[p.name]) save(p.name); if (e.key === "Escape") { e.stopPropagation(); closeKey(p.name); } }} aria-label={`${p.name} key`}
            placeholder={p.key ? "Paste a new key" : `Paste a key${p.example ? ` (${p.example})` : ""}`} autoComplete="off" spellCheck={false} className={`w-56 ${input}`} />
          <button disabled={!keys[p.name]} onClick={() => save(p.name)} className={button}>Save</button>
          <button onClick={() => closeKey(p.name)} className={quietButton}>Cancel</button>
        </> : <>
          {/* the same key on Mistral and Mistral Vibe: the row's line asks which it is; yes keeps it here (nothing typed is sent)
            and takes it off the other. Kept from before under Mistral Vibe: yes confirms it there, remove takes it out */}
          {p.unsure === "pair" && <button onClick={() => save(p.name, "")} data-key-for={p.name} className={button}>Yes</button>}
          {p.unsure === "old_vibe" && <>
            <button onClick={() => save("Mistral Vibe", "")} data-key-for={p.name} className={button}>Yes</button>
            <button onClick={() => { core.removeKey("Mistral Vibe").catch(() => {}); refresh(); }} className={button}>Remove it</button>
          </>}
          <button onClick={() => setKeyFor(p.name)} className={button}>{p.key ? "Replace key" : "Add key"}</button>
        </>}
        <button onClick={() => core.openKeyPage(p.page)} aria-label={`Make a key in ${p.place}`} title={`Make a key in ${p.place}`} className="rounded p-1 text-muted hover:text-ink"><ExternalLink size={14} strokeWidth={1.5} /></button>
      </>}
    </Row>
  );
  return (
    <div onMouseDown={onClose} className="tz-fade fixed inset-0 z-30 grid place-items-center bg-scrim p-10">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Settings" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className={`${dialogShell(860)} h-[620px] flex-row!`}>
        <nav aria-label="Settings" className="flex w-[196px] flex-none flex-col gap-px border-r border-line-soft bg-sidebar px-2 pb-3 pt-2.5">
          <div className="px-2.5 pb-2.5 pt-px font-medium">Settings</div>
          {["Appearance", "Providers", "Privacy and routing", "Paid API spend", "Tools from MCP servers", ...(hook ? ["After each turn"] : []), "Quick chat", "Panels and plugins", "About"].map((t) => (
            <button key={t} onClick={() => setTab(t)} aria-current={t === tab ? "page" : undefined}
              className={`rounded-md px-2.5 py-[5px] text-left text-[13px] ${t === tab ? "bg-selected font-medium text-ink" : "text-ink-2"}`}>{t}</button>
          ))}
        </nav>
        <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex items-center border-b border-line-soft py-2.5 pl-6 pr-4">
          <span className="flex-1 font-medium">{tab}</span>
          <button onClick={onClose} title="Close (Esc)" aria-label="Close" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="flex-1 overflow-auto bg-main px-6 pb-7 pt-5">
          <TabCtx.Provider value={tab}>
          <Section title="Appearance">
            {/* the labels and the controls say it: no line under each (design review 14) */}
            <Row title="Theme">
              <Segmented label="Theme" value={looks.theme} options={[["system", "System"], ["light", "Light"], ["dark", "Dark"]]} onChange={(v) => setLook(THEME, "theme", v)} />
            </Row>
            {/* design review 13 (D2): the accents, each drawn for both themes; a swatch shows the theme's own (styles.css) */}
            <Row title="Accent">
              <Swatches label="Accent" value={looks.accent} options={ACCENTS} onChange={(v) => setLook(ACCENT, "accent", v)} />
            </Row>
            <Row title="Text size">
              <Segmented label="Text size" value={looks.zoom} options={TEXT_SIZES} onChange={(v) => setLook(ZOOM, "zoom", v)} />
            </Row>
            <Row title="Answers">
              <Segmented label="Answers" value={looks.prose} options={[["serif", "Serif"], ["sans", "Sans"]]} onChange={(v) => setLook(PROSE, "prose", v)} />
            </Row>
            {/* the work panel's Stats: tiles, or rows on the desk (design review 14, S4 of mockups/v12.html) */}
            <Row title="Stats">
              <Segmented label="Stats" value={looks.stats} options={[["tiles", "Tiles"], ["rows", "Rows"]]} onChange={(v) => setLook(STATS, "stats", v)} />
            </Row>
            {/* the conversation's numbers: the composer's strip, or the status bar, or nowhere (design review 14, mockups/v13.html) */}
            <Row title="Conversation numbers">
              <Segmented label="Conversation numbers" value={looks.numbers} options={[["under", "Under the box"], ["above", "Above the box"], ["bar", "In the status bar"], ["off", "Off"]]}
                onChange={(v) => setLook(NUMBERS, "numbers", v)} />
            </Row>
            {/* they and the Stats count every frame while a reply streams, or move as each model call ends */}
            <Row title="Live stats">
              <Segmented label="Live stats" value={looks.live} options={[["on", "On"], ["off", "Off"]]} onChange={(v) => setLook(LIVE, "live", v)} />
            </Row>
          </Section>
          {!st && tab !== "Appearance" && <div className="text-[12.5px] text-faint">Loading…</div>}
          {st && <>
            <Section title="Providers" label="Yours" note="Claude runs on your subscription; other providers take a key or a sign-in.">
              <Row title="Claude" note={<ClaudeNote s={claude} />}><ClaudeButtons s={claude} check={checkClaude} checking={checkingClaude} /></Row>
              {yours.length ? yours.map(provider) : <div className="px-4 py-2.5 text-[12.5px] text-faint">No keys yet: add one under More providers.</div>}
            </Section>
            <Section title="Providers" label={<button onClick={() => setMore(!more)} aria-expanded={more} className="-mx-1.5 flex items-center gap-1 rounded-md px-1.5 py-0.5 hover:text-ink">
              <ChevronRight size={14} strokeWidth={1.5} className={`text-faint transition-transform ${more ? "rotate-90" : ""}`} />More providers<span className="font-normal text-faint">{others.length}</span></button>}>
              {more && <>
                <div className="flex items-center gap-2 px-4 py-2.5 text-muted">
                  <Search size={14} strokeWidth={1.5} />
                  <input value={find} onChange={(e) => setFind(e.target.value)} placeholder="Search by name or host" aria-label="Search providers" spellCheck={false}
                    className="flex-1 bg-transparent text-[13px] text-ink outline-none placeholder:text-faint" />
                </div>
                {found.map(provider)}
                {!found.length && <div className="px-4 py-2.5 text-[12.5px] text-faint">No provider matches.</div>}
              </>}
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
                  <button onClick={() => setAskAny(false)} className={quietButton}>Cancel</button>
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

            <Section title="Tools from MCP servers" note={<>Servers for every model, from tazuna's list. A server runs its command on this machine: add only ones you trust.</>}>
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
              <Section title="After each turn" note={chat ? <>A command of yours for a project, such as a formatter, run after each turn that changed files.
                It runs at no token cost: the model never sees it, and only a failure shows.</>
                : <>A command of yours for <span className="font-mono">{hook.cwd}</span>, such as a formatter. It runs there at no token cost: the model
                never sees it, and only a failure shows. Kept in your settings, never the repository's.</>}>
                {/* a chat runs in the home folder: no command for it (it offered one "for /private/tmp/…/home", design review 14) */}
                {chat ? <div className="px-4 py-3 text-[12.5px] text-muted">For projects only: open a project, then set its command here.</div> : <>
                <div className="flex items-center gap-2 px-4 py-3">
                  <input value={hookCmd ?? hook.after_turn ?? ""} onChange={(e) => setHookCmd(e.target.value)} placeholder="none, e.g. cargo fmt" aria-label="Command after each turn"
                    spellCheck={false} onKeyDown={(e) => { if (e.key === "Enter" && hookCmd != null) core.hook(hookCmd.trim()).then(() => setHookCmd(null)); }} className={`min-w-0 flex-1 ${input}`} />
                  <button disabled={hookCmd == null} onClick={() => core.hook(hookCmd!.trim()).then(() => setHookCmd(null))} className={button}>Save</button>
                </div>
                {hook.note && <div className="px-4 py-2.5 text-[12px] text-bad">{hook.note}</div>}
                </>}
              </Section>
            )}

            <Section title="Quick chat">
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
                <Row key={w.id} title={w.title} note={<Parts className="block truncate" items={[w.kind === "panel" ? "Panel, sandboxed" : w.state === "enabled" ? "Plugin, on"
                  : w.state === "changed" ? "Plugin: its code changed since you enabled it, look before enabling again" : "Plugin, off", w.about]} />}>
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
                    <button onClick={() => setReading(null)} className={quietButton}>Cancel</button>
                    <button onClick={() => { onEnable?.({ ...reading.w, print: reading.print }, true); setReading(null); }}
                      className={accentButton}>Enable this code</button>
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
                  <Row key={g.file} title={<>{g.title} <Parts className="text-[11.5px] font-normal text-faint" items={[g.kind, g.author && `by ${g.author}`]} /></>} note={g.about}>
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
                note={<>The app carries its own tazuna (<span className="tabular-nums">{st.version}</span>), apart from the one in your terminal. It looks for updates every hour.</>}>
                {update.version
                  ? update.installing ? <span role="status" className="text-[12px] text-muted">{progressText(update)}</span>
                    : <button onClick={() => installUpdate()} title="Replies running now stop; conversations stay resumable"
                        className={accentButton}>Install {update.version} and restart</button>
                  // aria-disabled, not disabled, while checking: a disabled button drops focus, which then had nowhere to
                  // come back to when the answer's dialog closed
                  : <button aria-disabled={checking} onClick={async () => { if (checking) return; setChecking(true); await checkUpdate(true); setChecking(false); }}
                      className={`flex items-center gap-1.5 ${button} aria-disabled:opacity-60`}><RefreshCw size={12} strokeWidth={1.75} className={checking ? "animate-spin" : ""} aria-hidden="true" />{checking ? "Checking…" : "Check for updates"}</button>}
              </Row>
            </Section>
          </>}
          </TabCtx.Provider>
        </div>
        </div>
      </div>
      {welcome && <PlanWelcome onClose={() => setWelcome(false)} />}
    </div>
  );
}

/** The first sign-in that uses the ChatGPT plan, once (OpenAI's guidelines for ChatGPT plan usage): what it means, and
 * where its usage is managed. */
function PlanWelcome({ onClose }: { onClose: () => void }) {
  const dialog = useDialog();
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    // before Settings' own Escape, which would close the whole page
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopPropagation(); e.preventDefault(); close.current(); } };
    window.addEventListener("keydown", esc, true);
    return () => window.removeEventListener("keydown", esc, true);
  }, []);
  return (
    <div onMouseDown={(e) => { e.stopPropagation(); onClose(); }} className="tz-fade fixed inset-0 z-40 grid place-items-center bg-scrim p-10">
      <div ref={dialog} role="dialog" aria-modal="true" aria-labelledby="plan-welcome" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className={`${dialogShell(420)} p-5`}>
        <div id="plan-welcome" className="text-[14px] font-semibold text-ink">You're using your ChatGPT plan</div>
        <p className="mt-1 text-[12.5px] leading-relaxed text-muted">Requests on ChatGPT's models in tazuna use your ChatGPT plan. You can see that usage and set its limits in
          ChatGPT's settings.</p>
        <div className="mt-5 flex justify-end gap-2">
          <button onClick={() => core.openLink("https://chatgpt.com/settings/usage").catch(() => {})} className={quietButton}>Manage usage</button>
          <button autoFocus onClick={onClose} className={accentButton}>Got it</button>
        </div>
      </div>
    </div>
  );
}

/** A part of the page: its heading, a line on what it is for, and its rows in one card, a line between each. */
function Section({ title, note, label, children }: { title: string; note?: React.ReactNode; label?: React.ReactNode; children: React.ReactNode }) {
  // the part chosen on the left is the one on screen; its name is the page's heading
  if (useContext(TabCtx) !== title) return null;
  return (
    // a part may come in pieces, each under its own `label` (Providers: yours, then the rest)
    <section className="mt-6 first:mt-0">
      {note && <p className="mb-3 text-[12.5px] leading-relaxed text-muted">{note}</p>}
      {label && <div className="mb-2 text-[12.5px] font-medium text-ink-2">{label}</div>}
      {children && <div className="divide-y divide-line-soft rounded-lg border border-line bg-card">{children}</div>}
    </section>
  );
}

/** One setting: its name and what it does on the left, its control on the right. */
function Row({ title, note, children, below }: { title: React.ReactNode; note?: React.ReactNode; children?: React.ReactNode;
  /** a line of its own under the row, the card's width (a provider's usage) */
  below?: React.ReactNode }) {
  return (
    // the control goes under its label where the row is too narrow for both (Larger text): it ran past the card
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
      <div className="min-w-[150px] flex-1">
        <div className="text-[13px] font-medium text-ink">{title}</div>
        {note && <div className="mt-0.5 text-[12px] leading-relaxed text-muted">{note}</div>}
      </div>
      {children && <div className="flex flex-none items-center gap-2">{children}</div>}
      {below && <div className="min-w-0 basis-full">{below}</div>}
    </div>
  );
}

// A provider's usage line (review 14, usage sources; its fix after the user's review): the Track switch, how it is read (a
// choice where there are several), one short state and the one thing to do for it; the method's steps on the state's
// hover. An admin key is never shown: it tells a month's spend, not the plan's use, and offered nothing to do. Every
// request waits on its own reply: its button says so, and a second click meanwhile does nothing.
const METHOD: Record<UsageMethod["id"], string> = { api: "API", api_scope: "API", admin_key: "Admin key", cli: "CLI", console: "Console", local: "Count here", none: "None" };
const UNITS: [Allowance["unit"], string][] = [["tokens", "Tokens"], ["usd", "USD"], ["requests", "Requests"]];
const PERIODS: [Allowance["period"], string][] = [["day", "Day"], ["week", "Week"], ["month", "Month"]];
// a week's first day as the core counts it: 0 Sunday to 6 Saturday, listed from Monday
const WEEKDAYS: [number, string][] = [[1, "Monday"], [2, "Tuesday"], [3, "Wednesday"], [4, "Thursday"], [5, "Friday"], [6, "Saturday"], [0, "Sunday"]];
/** A reset that fits the period: a month's day 1-31, a week's 0-6 (Monday where the old one does not fit), a day's 0. */
const resetFor = (period: Allowance["period"], r: number | null) =>
  period === "day" ? 0 : period === "week" ? (r != null && r >= 0 && r <= 6 ? r : 1) : r != null && r >= 1 && r <= 31 ? r : 1;

function UsageLine({ u, learned, busy, track, check, allow, note, say, refreshLearned }: { u: UsageSource; learned: Learned | null | undefined;
  busy: { track: boolean; check: boolean; save: boolean };
  track: (u: UsageSource, on: boolean, method: string) => void; check: (u: UsageSource) => void; allow: (u: UsageSource, a: Allowance | null) => void;
  refreshLearned: (provider: string) => void; note: { text: string; bad?: boolean } | null; say: (text: string, bad?: boolean) => void }) {
  const ways = u.methods.filter((x) => x.id !== "admin_key");
  const m = ways.find((x) => x.id === u.track.method) ?? ways[0];
  const [copied, setCopied] = useState("");
  // the console window's open and Forget: Tauri commands that answer when done
  const [doing, setDoing] = useState<"" | "open" | "forget">("");
  const [form, setForm] = useState<Allowance>(() => (u.allowance ? { ...u.allowance, reset: resetFor(u.allowance.period, u.allowance.reset) } : { amount: 0, unit: "tokens", period: "month", reset: 1 }));
  if (!m) return null;
  const st = usageState(m, u, learned);
  const copy = (c: string) => navigator.clipboard?.writeText(c).then(() => { setCopied(c); setTimeout(() => setCopied(""), 1400); }, () => {});
  const field = "rounded-md border border-line bg-field px-2 py-1 text-[12.5px] tabular-nums text-ink outline-none focus:border-accent";
  // a day's reset is 0 here and may be null from the core: the same
  const same = (a: Allowance | null) => !!a && a.amount === form.amount && a.unit === form.unit && a.period === form.period && (a.reset ?? 0) === (form.reset ?? 0);
  const valid = form.amount > 0 && Number.isFinite(form.amount) && resetFor(form.period, form.reset) === (form.reset ?? 0);
  const console_ = (what: "open" | "forget") => {
    if (doing) return;
    setDoing(what);
    (what === "open" ? core.consoleOpen(u.provider, true) : core.consoleForget(u.provider).then(() => { say("Signed out of its console."); refreshLearned(u.provider); }))
      .catch((e) => say(`${what === "open" ? "Could not open its console" : "Not forgotten"}: ${e}`, true)).finally(() => setDoing(""));
  };
  // a button whose request is in flight: its words say so, and it takes no second click (aria-disabled keeps its focus)
  const act = (label: string, waiting: string, on: boolean, run: () => void, disabled = false) => (
    <button aria-disabled={on || disabled} disabled={disabled} onClick={() => { if (!on) run(); }} className={`flex items-center gap-1.5 ${quietButton} aria-disabled:opacity-60`}>
      {on && <LoaderCircle size={12} strokeWidth={2} className="animate-spin" aria-hidden="true" />}{on ? waiting : label}</button>
  );
  const isConsole = u.track.on && m.id === "console";
  const forget = (label: string) => act(label, "Forgetting…", doing === "forget", () => console_("forget"), doing === "open");
  return (
    <div className="flex flex-col gap-2 text-[12.5px]">
      <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1.5">
        <span className="text-faint">Usage</span>
        <Switch on={u.track.on} busy={busy.track} onChange={(on) => track(u, on, m.id)} label={`Track ${u.provider}'s usage`} />
        {/* untracked: the switch alone (and its wait, turning it off) */}
        {!u.track.on && busy.track && <LoaderCircle size={12} strokeWidth={2} className="animate-spin text-faint" aria-label="Saving" />}
        {u.track.on && <>
          {ways.length > 1
            ? <Segmented label={`How tazuna reads ${u.provider}'s usage`} value={m.id} options={ways.map((x) => [x.id, METHOD[x.id]] as const)} onChange={(id) => track(u, true, id)} />
            : <span className="text-ink-2">{METHOD[m.id]}</span>}
          {busy.track && <LoaderCircle size={12} strokeWidth={2} className="animate-spin text-faint" aria-label="Saving" />}
          {/* it narrows and wraps beside its button rather than send the button to a line of its own */}
          {st.text && <span title={m.steps || undefined} className="min-w-0 flex-[1_1_7rem] text-muted">{st.text}</span>}
        </>}
        {u.track.on && <span className="ml-auto flex flex-none items-center gap-2">
          {st.action === "check" && act("Check again", "Checking…", busy.check, () => check(u))}
          {st.action === "use" && m.instead && <button onClick={() => track(u, true, m.instead!)} className={quietButton}>Use {METHOD[m.instead]}</button>}
          {(st.action === "signin" || st.action === "open") && act(st.action === "signin" ? "Sign in…" : "Open again", "Opening…", doing === "open", () => console_("open"), doing === "forget")}
        </span>}
      </div>
      {/* the sign-in's line: what it risks, and its sign-in to forget (two buttons beside the state sent both under it); read
          another way or not at all, its sign-in can go all the same */}
      {isConsole && <div className="flex items-center gap-2"><p className="min-w-0 flex-1 text-faint">Uses your sign-in; may break the provider's terms.</p>
        {learned?.store && forget("Forget sign-in")}</div>}
      {u.track.on && !!st.commands?.length && <div className="flex flex-wrap items-center gap-2">
        {st.commands.map((c) => <span key={c} className="flex items-center gap-1">
          <code className="rounded-md bg-field px-2 py-1 font-mono text-[11.5px] text-ink-2">{c}</code>
          <button onClick={() => copy(c)} aria-label={`Copy ${c}`} title="Copy" className="rounded p-1 text-muted hover:text-ink">
            {copied === c ? <Check size={13} strokeWidth={1.75} className="text-ok" /> : <Copy size={13} strokeWidth={1.5} />}</button>
        </span>)}
      </div>}
      {u.track.on && m.id === "local" && <div className="flex flex-wrap items-center gap-2">
        <input type="number" min={0} step="any" value={form.amount || ""} onChange={(e) => setForm({ ...form, amount: Number(e.target.value) })} placeholder="Allowance"
          aria-label={`${u.provider}'s allowance`} className={`w-28 ${field} placeholder:text-faint`} />
        {/* the system's own menus: three Segmented keys each ran the form onto a second line */}
        <select aria-label="Unit" value={form.unit} onChange={(e) => setForm({ ...form, unit: e.target.value as Allowance["unit"] })} className={field}>
          {UNITS.map(([v, l]) => <option key={v} value={v}>{l}</option>)}</select>
        <span className="text-muted">a</span>
        <select aria-label="Period" value={form.period} onChange={(e) => { const period = e.target.value as Allowance["period"]; setForm({ ...form, period, reset: resetFor(period, form.reset) }); }} className={field}>
          {PERIODS.map(([v, l]) => <option key={v} value={v}>{l}</option>)}</select>
        {form.period === "month" && <label className="flex items-center gap-1.5 text-muted">from day
          <input type="number" min={1} max={31} value={form.reset || ""} onChange={(e) => setForm({ ...form, reset: Number(e.target.value) })} className={`w-14 ${field}`} /></label>}
        {form.period === "week" && <label className="flex items-center gap-1.5 text-muted">from
          <select aria-label="Resets on" value={form.reset ?? 1} onChange={(e) => setForm({ ...form, reset: Number(e.target.value) })} className={field}>
            {WEEKDAYS.map(([v, d]) => <option key={d} value={v}>{d}</option>)}</select></label>}
        {act("Save", "Saving…", busy.save, () => allow(u, form), !busy.save && (same(u.allowance) || !valid))}
        {u.allowance && !busy.save && <button onClick={() => { setForm({ amount: 0, unit: "tokens", period: "month", reset: 1 }); allow(u, null); }} className={quietButton}>Clear</button>}
      </div>}
      {!isConsole && learned?.store && <div className="flex items-center gap-2"><p className="min-w-0 flex-1 text-faint">Console sign-in kept</p>{forget("Forget")}</div>}
      {note && <p role="status" className={note.bad ? "text-bad" : "text-ink-2"}>{note.text}</p>}
    </div>
  );
}

/** On or off; `busy` while its change is being saved: a click then does nothing. */
function Switch({ on, onChange, label, busy }: { on: boolean; onChange: (on: boolean) => void; label: string; busy?: boolean }) {
  return (
    <button role="switch" aria-checked={on} aria-label={label} aria-busy={busy || undefined} onClick={() => { if (!busy) onChange(!on); }}
      className={`relative h-5 w-9 flex-none rounded-full ${on ? "bg-accent" : "bg-line-strong"}`}>
      <span className={`absolute left-0.5 top-0.5 size-4 rounded-full bg-white shadow-sm transition-transform duration-150 ${on ? "translate-x-4" : ""}`} />
    </button>
  );
}

