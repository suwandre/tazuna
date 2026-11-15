import { useEffect, useState } from "react";
import { Composer } from "./components/Composer";
import { Rail, builtInWidgets } from "./components/Rail";
import { Sidebar } from "./components/Sidebar";
import { Transcript } from "./components/Transcript";
import { chats, messages, projects, session, usage } from "./lib/demo";

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
  return (
    <div className="grid h-screen grid-cols-[248px_1fr_232px]">
      <Sidebar chats={chats} projects={projects} active="t1" planLeft="Max · 62% left" />
      <main className="flex min-w-0 flex-col bg-main">
        <header data-tauri-drag-region className="flex items-center justify-between border-b border-line-soft px-7 py-3">
          <div className="flex items-center gap-2 font-medium"><span className="text-muted">api-gateway</span><span className="text-faint">/</span><span>Retry test fails on CI</span></div>
          <div className="flex gap-1.5">
            <span className="rounded-full border border-line bg-field px-2.5 py-1 text-[12px] text-ink-2">{session.model} · {session.effort}</span>
            <span className="rounded-full border border-line px-2.5 py-1 text-[12px] text-ink-2">{session.branch} · {session.changed} changed</span>
          </div>
        </header>
        <Transcript messages={messages} />
        <Composer session={session} threadCost={usage.threadCost} />
      </main>
      <Rail usage={usage} widgets={builtInWidgets} />
    </div>
  );
}
