import { ExternalLink, X } from "lucide-react";
import { useEffect, useState } from "react";
import { core, type Settings } from "../lib/tazuna";

// Settings: provider keys (saved by tazuna, never shown again), the data policy for this session, the router. The
// terminal's /login, /data and /route in one page.
export function SettingsView({ onClose }: { onClose: () => void }) {
  const [st, setSt] = useState<Settings | null>(null);
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [askAny, setAskAny] = useState(false);
  const refresh = () => core.settings().catch(() => {});
  useEffect(() => {
    const off = core.onSettings(setSt);
    refresh();
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", esc);
    return () => { off.then((f) => f()); window.removeEventListener("keydown", esc); };
  }, [onClose]);
  const save = async (name: string) => {
    await core.login(name, keys[name] ?? "");
    setKeys((k) => ({ ...k, [name]: "" }));
    refresh(); core.models().catch(() => {});
  };
  const label = "mb-2 mt-5 text-[11.5px] font-medium text-faint first:mt-0";
  return (
    <div onMouseDown={onClose} className="fixed inset-0 z-30 grid place-items-center bg-ink/15 p-10">
      <div onMouseDown={(e) => e.stopPropagation()} className="flex max-h-full w-[640px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
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

            <div className="mt-5 text-[11.5px] text-faint">tazuna {st.version}</div>
          </>}
        </div>
      </div>
    </div>
  );
}
