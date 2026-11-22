import { Fragment, type ReactNode } from "react";

// Items of one line apart: a hairline between them, not a middle dot. "a · b · c" in every second line, chip and caption
// read as clutter (design review 6); where the layout can say it (a stat under its value, a column), it does instead.

/** A thin vertical line between two items of a line. */
export const Sep = () => <span aria-hidden="true" className="mx-1.5 inline-block h-[0.85em] w-px translate-y-[0.1em] bg-line-strong" />;

/** The items that are there, a hairline between each two; a string from the core split where it has " · ". */
export function Parts({ items, className }: { items: (ReactNode | string | null | undefined | false)[]; className?: string }) {
  const shown = items.flatMap((x) => (typeof x === "string" ? x.split(" · ") : [x])).filter((x) => x !== null && x !== undefined && x !== false && x !== "");
  return <span className={className}>{shown.map((x, i) => <Fragment key={i}>{i > 0 && <Sep />}{x}</Fragment>)}</span>;
}

/** Lines added and removed, "+3 −1", in the result colours; a side with nothing is left out, never "+3 −0" (design review
 * 12). */
export const Counts = ({ added, removed, className = "" }: { added: number; removed: number; className?: string }) => <>
  {added > 0 && <span className={`tabular-nums text-ok ${className}`}>+{added}</span>}{added > 0 && removed > 0 && " "}
  {removed > 0 && <span className={`tabular-nums text-bad ${className}`}>−{removed}</span>}
</>;
