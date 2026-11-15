import { useEffect, useState } from "react";
import { Composer } from "./components/Composer";
import { Rail, builtInWidgets } from "./components/Rail";
import { Sidebar } from "./components/Sidebar";
import { Transcript } from "./components/Transcript";
import * as demo from "./lib/demo";
import { messages as viewMessages, usage as viewUsage } from "./lib/fromView";
import { core, inTauri, type View } from "./lib/tazuna";

/** The core's latest view, in the Tauri window; null in a plain browser (the demo data shows instead). */
function useCore() {
  const [view, setView] = useState<View | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!inTauri) return;
    const offs: Promise<() => void>[] = [
      core.onExit(() => setError("tazuna stopped. The app needs tazuna 0.1.128 or later on the PATH (or TAZUNA_BIN).")),
    ];
    const view = core.onView((v) => { setError(null); setView(v); });
    offs.push(view);
    view.then(() => core.start(undefined, "chat")).catch((e) => setError(String(e)));
    return () => offs.forEach((p) => p.then((off) => off()));
  }, []);
  return { view, error };
}

function useTheme() {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const [dark, setDark] = useState(media.matches);
  useEffect(() => {
    const on = (e: MediaQueryListEvent) => setDark(e.matches);
    media.addEventListener("change", on);
    return () => media.removeEventListener("change", on);
  }, [media]);
  useEffect(() => { document.documentElement.classList.toggle("dark", dark); }, [dark]);
  // Cmd/Ctrl-Shift-L flips light and dark for this window
  useEffect(() => {
    const key = (e: KeyboardEvent) => { if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "l") setDark((d) => !d); };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);
}

export default function App() {
  useTheme();
  const { view, error } = useCore();
  const messages = view ? viewMessages(view) : inTauri ? [] : demo.messages;
  const usage = view ? viewUsage(view) : demo.usage;
  const busy = !!view && !["Ready", "Done"].includes(view.activity);
  const session = view
    ? { model: view.model || "default", effort: view.effort || "", mode: view.mode, branch: "", changed: 0, lastCost: lastCost(view) }
    : demo.session;
  const folder = view?.cwd.split("/").filter(Boolean).pop() ?? "api-gateway";
  const title = view ? (view.blocks.find((b) => b.kind === "prompt") as { text?: string } | undefined)?.text?.slice(0, 60) ?? "New chat" : "Retry test fails on CI";
  return (
    <div className="grid h-screen grid-cols-[248px_1fr_232px]">
      <Sidebar chats={view ? [] : demo.chats} projects={view ? [] : demo.projects} active="t1" planLeft={view?.limits ? `${Math.round(100 - view.limits[0])}% left` : "Max · 62% left"} />
      <main className="flex min-w-0 flex-col bg-main">
        <header data-tauri-drag-region className="flex items-center justify-between border-b border-line-soft px-7 py-3">
          <div className="flex items-center gap-2 font-medium"><span className="text-muted">{folder}</span><span className="text-faint">/</span><span className="truncate">{title}</span></div>
          <div className="flex gap-1.5">
            <span className="rounded-full border border-line bg-field px-2.5 py-1 text-[12px] text-ink-2">{session.model} · {session.effort}</span>
            {session.branch && <span className="rounded-full border border-line px-2.5 py-1 text-[12px] text-ink-2">{session.branch} · {session.changed} changed</span>}
          </div>
        </header>
        <Transcript messages={messages} busy={busy} activity={view?.activity} />
        {error && <div className="mx-auto mb-2 max-w-[700px] px-7 text-[12.5px] text-bad">{error}</div>}
        <Composer session={session} threadCost={usage.threadCost} busy={busy}
          onSend={(text) => core.send(text).catch((e) => console.error(e))} onStop={() => core.interrupt()} />
      </main>
      <Rail usage={usage} widgets={builtInWidgets} />
    </div>
  );
}

/** The cost of the last finished turn. */
function lastCost(v: View): number {
  for (let i = v.blocks.length - 1; i >= 0; i--) { const b = v.blocks[i]; if (b.kind === "done") return b.cost; }
  return 0;
}
