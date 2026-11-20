// tazuna's mark (design review 10, mockups/v7.html): 手 (te), the hand of 手綱 (tazuna, the reins), drawn in the icons' own
// round stroke, its last stroke ending in a rein's loop instead of the hook. The same drawing is the app icon
// (desktop/app-icon.svg) and, as two cells, the terminal's welcome.

export const MARK = <><path d="M16.2 4.6Q12.4 6 7.6 6.4" /><path d="M7.2 10.2h9.6M5.4 14.2h13.2" /><path d="M12 6.2v11.3a2 2 0 1 1-2.6-1.9" /></>;

/** The mark on its indigo tile, `size` px square. */
export function Mark({ size = 22 }: { size?: number }) {
  const inner = Math.round(size * 0.8);
  return (
    <span aria-hidden="true" className="grid flex-none place-items-center rounded-[22%] bg-accent text-on-accent" style={{ width: size, height: size }}>
      <svg viewBox="0 0 24 24" width={inner} height={inner} fill="none" stroke="currentColor" strokeWidth={size <= 24 ? 2.2 : 1.9} strokeLinecap="round" strokeLinejoin="round">{MARK}</svg>
    </span>
  );
}
