const strokes = [
  "M5 5h3V4h3v1H8v1H5z",
  "M5 7h3v1H5z",
  "M4 9h4v1H4z",
  "M9 6h1v6H6v-1h3z",
].map((d) => <path key={d} d={d} />);

export function Mark({ size = 20, motion = "still" }: { size?: number; motion?: "still" | "write" | "pulse" }) {
  return (
    <span aria-hidden="true" className={`tz-mark tz-mark-${motion} grid flex-none place-items-center rounded-[22%]`} style={{ width: size, height: size }}>
      <svg viewBox="0 0 16 16" width={size} height={size} fill="currentColor">
        {motion === "write" && <g className="tz-mark-guide">{strokes}</g>}
        <g className="tz-mark-strokes">{strokes}</g>
      </svg>
    </span>
  );
}
