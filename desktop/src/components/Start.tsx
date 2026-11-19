import { CircleAlert, FolderOpen, MessageSquare, Plus, RotateCcw } from "lucide-react";
import type { ReactNode } from "react";
import { age } from "./Sidebar";

// A new conversation before its first message (the second design review, prototype mockups/v3.html): the message box in
// the middle under a question, a way back into recent work and into a project, the keys worth knowing. Until desktop
// 0.1.68 it was an empty page with the box at the bottom.

/** The core of a conversation did not start: why, and what to do (desktop 0.1.77; it was a red line under demo numbers). */
export function CoreFailed({ error, folder, onRetry, onChat }: { error: string; folder: string | null; onRetry: () => void; onChat: () => void }) {
  return (
    <div className="flex min-h-0 flex-1 flex-col items-center overflow-auto px-7 py-10">
      <div role="alert" className="my-auto w-full max-w-[560px] pb-[6vh]">
        <div className="flex items-start gap-3 rounded-xl border border-bad/35 bg-bad/[0.06] px-5 py-4">
          <CircleAlert size={18} strokeWidth={1.5} className="mt-0.5 flex-none text-bad" />
          <div className="min-w-0 flex-1">
            <h2 className="text-[15px] font-semibold text-ink">tazuna could not start {folder ? "in this project" : "a chat"}</h2>
            <p className="mt-1 text-[13px] text-ink-2">The window runs the tazuna command for each conversation, and it did not come up. Check that it runs in a
              terminal (<code className="font-mono text-[12px]">tazuna --version</code>), then try again.</p>
            <pre className="mt-2.5 whitespace-pre-wrap break-words rounded-md bg-field px-2.5 py-2 font-mono text-[11.5px] text-muted">{error}</pre>
            <div className="mt-3 flex gap-2">
              <button onClick={onRetry} className="flex items-center gap-1.5 rounded-md bg-accent px-3 py-1.5 text-[12.5px] font-medium text-on-accent"><RotateCcw size={13} strokeWidth={1.75} />Try again</button>
              {folder && <button onClick={onChat} className="flex items-center gap-1.5 rounded-md border border-line px-3 py-1.5 text-[12.5px] text-ink hover:bg-hover"><MessageSquare size={13} strokeWidth={1.5} />A chat instead</button>}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export type StartItem = { id: string; title: string; place: string; dir: string | null; at: number };
export type StartProject = { path: string; name: string; count: number };

export function Start({ place, composer, recent, projects, onResume, onProject, onFolder }: {
  /** the project's name, or null in a chat */
  place: string | null;
  composer: ReactNode; recent: StartItem[]; projects: StartProject[];
  onResume: (s: StartItem) => void; onProject: (path: string) => void; onFolder: () => void;
}) {
  const row = "flex w-full items-center gap-2.5 border-t border-line-soft px-3.5 py-[7px] text-left hover:bg-hover focus-visible:outline-offset-[-2px]";
  const head = "flex px-3.5 pb-1.5 pt-2.5 text-[11.5px] font-medium text-faint";
  return (
    // the block in the middle of the page, a little above it (it sat 12% from the top, and a tall window was empty in its
    // lower half, desktop 0.1.75); my-auto, not justify-center, so a short window still scrolls to its top
    <div className="flex min-h-0 flex-1 flex-col items-center overflow-auto px-7 py-10">
      <div className="my-auto flex w-full flex-col items-center pb-[6vh]">
      <h2 className="mb-1.5 text-center font-prose text-[27px] font-normal leading-tight tracking-[-0.01em] text-ink">
        {place ? <>What are we working on in {place}?</> : "What are we working on?"}</h2>
      <div className="mb-3 text-center text-[12.5px] text-muted">{place ? "A change in this project, or a question about it." : "A question for a chat, or a change in one of your projects."}</div>
      <div className="w-full max-w-[756px]">{composer}</div>
      {(recent.length > 0 || projects.length > 0 || !place) && (
        <div className={`mt-2 grid w-full max-w-[700px] grid-cols-1 gap-3.5 ${recent.length > 0 ? "sm:grid-cols-2" : ""}`}>
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
    </div>
  );
}
