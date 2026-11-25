// tazuna's mark (design review 13, mockups/logo.html option E, the user's pick of six): 手 (ta, the hand of 手綱, the reins)
// as a 9×9 bitmap, a terminal's own kind of letter, its last stroke ending in the rein's loop; paper on ink. The same
// drawing is the app icon (desktop/app-icon.svg); the terminal's welcome keeps its two cells (▐手▌). Until desktop 0.1.124
// the mark was 手 in the icons' round stroke on indigo (mockups/marks/a-rein-loop-indigo.svg): logo.html keeps all six.

/** The bitmap's pixels in stroke order, so `working` writes it the way a hand does: ノ from its top right, the two
 * horizontals left to right (the stem's pixels on their rows with them), then the stem down and the loop at its foot. */
export const PIXELS: [number, number][] = [[7, 0], [6, 0], [5, 1], [4, 1], [3, 1],
  [1, 3], [2, 3], [3, 3], [4, 3], [5, 3], [6, 3], [7, 3],
  [0, 5], [1, 5], [2, 5], [3, 5], [4, 5], [5, 5], [6, 5], [7, 5], [8, 5],
  [4, 2], [4, 4], [4, 6], [4, 7], [4, 8], [3, 8], [2, 8], [1, 8], [1, 7]];

/** The mark on its ink tile, `size` px square, a multiple of 10 (a 10-unit grid: the 9 cells at a pitch of 1 and 0.75
 * wide, half a cell's margin round them; a 16 or 22 px tile blurred them). At 2x each cell is whole device pixels with a
 * pixel between them; at 1x a 20 px tile's cells meet and it reads as a solid bitmap (40 px keeps its gaps). `working`: the
 * pixels write themselves in stroke order, over and over, each dim until its turn in one shared cycle (all drawn with
 * reduced motion). */
export function Mark({ size = 20, working = false }: { size?: number; working?: boolean }) {
  return (
    <span aria-hidden="true" className="tz-mark grid flex-none place-items-center rounded-[22%]" style={{ width: size, height: size }}>
      <svg viewBox="0 0 10 10" width={size} height={size} fill="currentColor" shapeRendering="crispEdges">
        {PIXELS.map(([x, y], i) => (
          <rect key={i} x={0.5 + x} y={0.5 + y} width={0.75} height={0.75}
            className={working ? "tz-px" : undefined} style={working ? { ["--at" as string]: `${4 + Math.round((i * 66) / (PIXELS.length - 1))}%`,
              // where linear() is missing (styles.css): each pixel a tenth of a second behind the one before it
              ["--lag" as string]: `-${(((PIXELS.length - 1 - i) * 3) / PIXELS.length).toFixed(2)}s` } : undefined} />
        ))}
      </svg>
    </span>
  );
}
