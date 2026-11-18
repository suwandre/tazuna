import { FolderOpen, Plus } from "lucide-react";
import type { ReactNode } from "react";
import { age } from "./Sidebar";

// A new conversation before its first message (the second design review, prototype mockups/v3.html): the message box in
// the middle under a question, a way back into recent work and into a project, the keys worth knowing. Until desktop
// 0.1.68 it was an empty page with the box at the bottom.

export type StartItem = { id: string; title: string; place: string; dir: string | null; at: number };
export type StartProject = { path: string; name: string; count: number };

export function Start({ place, composer, recent, projects, onResume, onProject, onFolder }: {
  /** the project's name, or null in a chat */
  place: string | null;
  composer: ReactNode; recent: StartItem[]; projects: StartProject[];
  onResume: (s: StartItem) => void; onProject: (path: string) => void; onFolder: () => void;
}) {
  const row = "flex w-full items-center gap-2.5 border-t border-line-soft px-3.5 py-[7px] text-left hover:bg-hover";
  const head = "flex px-3.5 pb-1.5 pt-2.5 text-[11.5px] font-medium text-faint";
  return (
    <div className="flex min-h-0 flex-1 flex-col items-center overflow-auto px-7 pb-10 pt-[12vh]">
      <h2 className="mb-1.5 text-center font-prose text-[27px] font-normal leading-tight tracking-[-0.01em] text-ink">
        {place ? <>What are we working on in {place}?</> : "What are we working on?"}</h2>
      <div className="mb-3 text-center text-[12.5px] text-muted">{place ? "A change in this project, or a question about it." : "A question for a chat, or a change in one of your projects."}</div>
      <div className="w-full max-w-[756px]">{composer}</div>
      {(recent.length > 0 || projects.length > 0 || !place) && (
        <div className="mt-2 grid w-full max-w-[700px] grid-cols-1 gap-3.5 sm:grid-cols-2">
          {recent.length > 0 && (
            <section className="overflow-hidden rounded-xl border border-line bg-card">
              <div className={head}>Pick up where you left off</div>
              {recent.map((s) => (
                <button key={s.id} onClick={() => onResume(s)} className={row}>
                  <span className="min-w-0 flex-1"><span className="block truncate text-ink">{s.title}</span><span className="block truncate text-[11.5px] text-faint">{s.place}</span></span>
                  <span className="flex-none text-[11.5px] tabular-nums text-faint">{age(s.at)}</span>
                </button>
              ))}
            </section>
          )}
          <section className="overflow-hidden rounded-xl border border-line bg-card">
            <div className={head}>Start in a project</div>
            {projects.map((p) => (
              <button key={p.path} onClick={() => onProject(p.path)} title={p.path} className={row}>
                <FolderOpen size={14} strokeWidth={1.5} className="flex-none text-muted" />
                <span className="min-w-0 flex-1"><span className="block truncate text-ink">{p.name}</span>
                  <span className="block truncate text-[11.5px] text-faint">{p.path.replace(/^\/(Users|home)\/[^/]+/, "~")}{p.count ? ` · ${p.count} conversation${p.count === 1 ? "" : "s"}` : ""}</span></span>
              </button>
            ))}
            <button onClick={onFolder} className={row}>
              <Plus size={14} strokeWidth={1.5} className="flex-none text-muted" />
              <span className="min-w-0 flex-1"><span className="block text-ink">Open a folder…</span><span className="block text-[11.5px] text-faint">any project on this machine</span></span>
              <kbd className="flex-none rounded-[5px] border border-line px-[5px] font-mono text-[11px] text-faint">⌘O</kbd>
            </button>
          </section>
        </div>
      )}
      <div className="mt-5 flex gap-4 text-[11.5px] text-faint">
        {([["Search", "⌘K"], ["Mode", "⇧Tab"], ["Shortcuts", "⌘/"]] as const).map(([what, keys]) => (
          <span key={what}>{what} <kbd className="ml-1 rounded-[5px] border border-line px-[5px] font-mono text-[11px]">{keys}</kbd></span>
        ))}
      </div>
    </div>
  );
}
