// The window's shared controls (design review 11, prototype mockups/v8.html "one control, one button, one dialog"): each
// was drawn three ways (a lilac key in Settings, a raised one in the review, a bare one on the usage page; Cancel as plain
// words, bordered and filled; dialogs with three shadows and two body fills).

/** A switch between a few choices: a raised key on a field track. Lilac (`selected`) stays the selected row of a list.
 * A radio group for the keyboard too: Tab stops on the chosen key alone, the arrows (and Home, End) move and choose. */
export function Segmented<T extends string>({ value, options, onChange, label, wrap }: {
  value: T; options: (readonly [T, string])[]; onChange: (v: T) => void;
  /** what is being chosen, for a screen reader */
  label: string;
  /** more keys than a narrow place holds: they go on to a second row */
  wrap?: boolean;
}) {
  const at = options.findIndex(([v]) => v === value);
  return (
    <span role="radiogroup" aria-label={label} onKeyDown={(e) => radioKeys(e, options.map(([v]) => v), at, onChange)} className={`inline-flex flex-none rounded-[7px] bg-field p-0.5 text-[12.5px] ${wrap ? "flex-wrap" : ""}`}>
      {options.map(([v, l], i) => (
        <button key={v} role="radio" aria-checked={v === value} tabIndex={i === Math.max(0, at) ? 0 : -1} onClick={() => onChange(v)}
          className={`whitespace-nowrap rounded-[5px] px-[11px] py-[3px] ${v === value ? "bg-card font-medium text-ink shadow-[0_0_0_1px_var(--line-strong)]" : "text-muted hover:text-ink"}`}>{l}</button>
      ))}
    </span>
  );
}

/** A radio group's keys: the arrows, Home and End choose, and the focus goes with the choice. */
function radioKeys<T extends string>(e: React.KeyboardEvent<HTMLSpanElement>, options: readonly T[], at: number, onChange: (v: T) => void) {
  const n = options.length, to = e.key === "ArrowRight" || e.key === "ArrowDown" ? (at + 1) % n : e.key === "ArrowLeft" || e.key === "ArrowUp" ? (at + n - 1) % n : e.key === "Home" ? 0 : e.key === "End" ? n - 1 : -1;
  if (to < 0) return;
  e.preventDefault();
  onChange(options[to]);
  e.currentTarget.querySelectorAll<HTMLElement>("[role=radio]")[to]?.focus();
}

/** The accents to choose from (design review 13): round swatches, a radio group with Segmented's keys, each named by its
 * tooltip; the chosen one ringed in ink, clear of the card, and the focus ring outside that. Each is coloured by its own
 * `data-accent` (styles.css), so it follows a flip of light and dark with no render. */
export function Swatches<T extends string>({ value, options, onChange, label }: { value: T; options: readonly T[]; onChange: (v: T) => void; label: string }) {
  const at = options.indexOf(value);
  return (
    <span role="radiogroup" aria-label={label} onKeyDown={(e) => radioKeys(e, options, at, onChange)} className="inline-flex flex-none gap-[9px]">
      {options.map((v, i) => (
        <button key={v} role="radio" aria-checked={v === value} aria-label={v} title={v} tabIndex={i === Math.max(0, at) ? 0 : -1} onClick={() => onChange(v)} data-accent={v.toLowerCase()} style={{ background: "var(--accent)" }}
          className={`size-5 rounded-full focus-visible:outline-offset-[5px] ${v === value ? "shadow-[inset_0_0_0_1px_#0003,0_0_0_2px_var(--card),0_0_0_4px_var(--ink)]" : "shadow-[inset_0_0_0_1px_#0003]"}`} />
      ))}
    </span>
  );
}

/** The quiet button beside the one accent button (Cancel, Later, Close, Skip): bordered, on the card's fill. */
export const quietButton = "rounded-md border border-line-strong bg-card px-2.5 py-[3px] text-[12.5px] text-ink hover:bg-hover disabled:opacity-40";
/** The one accent button of a dialog or a bar. */
export const accentButton = "rounded-md border border-accent bg-accent px-2.5 py-[3px] text-[12.5px] font-medium text-on-accent disabled:opacity-40";

/** A dialog's frame: one of four widths (420 for a question or a short sheet), never wider or taller than the window less
 * its margin, the pop shadow; its body is the sheet's fill (`main`), its groups are cards with a hairline. The command
 * palette alone keeps a geometry of its own (560 px, near the window's top, at most 60% of its height). */
export const dialogShell = (width: 420 | 600 | 780 | 860) =>
  `tz-dialog flex max-h-[calc(100vh-5rem)] max-w-[calc(100vw-5rem)] flex-col overflow-hidden rounded-xl border border-line-strong bg-main ${width === 420 ? "w-[420px]" : width === 600 ? "w-[600px]" : width === 780 ? "w-[780px]" : "w-[860px]"}`;
