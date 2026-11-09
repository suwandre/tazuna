"""The README's charts, as SVG in a light and a dark version (docs/img/). Run: python3 docs/charts.py

Every number comes from a measured round; the source is named next to it (CHANGELOG versions, ROADMAP). A new round
changes the data here, then the script runs again. tazuna is the one series in color, everything it is compared with
is gray (emphasis): the charts are about tazuna against a reference, not about telling eight harnesses apart.
"""

import os
from xml.sax.saxutils import escape

THEMES = {
    # dataviz reference palette, slot 1 and the muted gray; surfaces are GitHub's page backgrounds (the SVGs are
    # transparent and sit on them). Validated: blue vs gray ΔE 15.9 under CVD, both ≥ 3:1 on either surface.
    "light": dict(ink="#0b0b0b", ink2="#52514e", muted="#6e6d68", grid="#e1e0d9", axis="#c3c2b7",
                  hi="#2a78d6", lo="#898781", ring="#ffffff"),
    "dark": dict(ink="#f0efec", ink2="#c3c2b7", muted="#9a9890", grid="#2a2e35", axis="#3d434c",
                 hi="#3987e5", lo="#898781", ring="#0d1117"),
}
FONT = "system-ui,-apple-system,'Segoe UI',Helvetica,Arial,sans-serif"
W = 760


def text(x, y, s, size=12, fill="ink", anchor="start", weight=400):
    return (f'<text x="{x:.1f}" y="{y:.1f}" font-size="{size}" font-weight="{weight}" text-anchor="{anchor}" '
            f'fill="{{{fill}}}">{escape(s)}</text>')


def header(title, sub, legend):
    out = [text(16, 26, title, 16, "ink", weight=600)]
    for i, line in enumerate(sub):
        out.append(text(16, 46 + 16 * i, line, 12, "ink2"))
    # legend, top right: a swatch beside each name (identity never rests on color alone)
    x = W - 16
    for name, color in reversed(legend):
        width = 7 * len(name) + 22
        x -= width
        out.append(f'<rect x="{x}" y="17" width="10" height="10" rx="2" fill="{{{color}}}"/>')
        out.append(text(x + 15, 26, name, 12, "ink2"))
    return out


def bars(title, sub, legend, groups, domain, ticks, unit_fmt, name):
    """Horizontal bars, grouped: groups = [(heading, [(label, value, color, tip), ...]), ...]."""
    left, right, top = 210, W - 24, 52 + 16 * len(sub)
    rows = sum(len(g[1]) for g in groups)
    height = top + 28 * len(groups) + 30 * rows + 36
    sx = lambda v: left + (right - 150 - left) * v / domain  # room past the longest bar for its label
    out = header(title, sub, legend)
    axis_y = height - 30
    for t in ticks:
        out.append(text(sx(t), axis_y + 16, unit_fmt(t), 11, "muted", "middle"))
    y = top + 6
    for heading, items in groups:
        y += 20
        out.append(text(16, y, heading, 12, "ink", weight=600))
        y += 8
        # the grid and the baseline run beside each group's bars only: a heading never has a line through it
        y1 = y + 30 * len(items)
        for t in ticks:
            out.append(f'<line x1="{sx(t):.1f}" x2="{sx(t):.1f}" y1="{y - 2}" y2="{y1}" stroke="{{{"axis" if t == 0 else "grid"}}}" stroke-width="1"/>')
        for label, value, color, tip in items:
            out.append(text(left - 10, y + 14, label, 12, "ink2", "end"))
            x0, x1, h, r = left, sx(value), 18, 4
            # 4px rounded data end, square at the baseline
            out.append(f'<path d="M{x0},{y + 2} H{x1 - r:.1f} A{r},{r} 0 0 1 {x1:.1f},{y + 2 + r} V{y + 2 + h - r} '
                       f'A{r},{r} 0 0 1 {x1 - r:.1f},{y + 2 + h} H{x0} Z" fill="{{{color}}}"/>')
            out.append(text(x1 + 6, y + 15, tip, 12, "ink"))
            y += 30
    return name, height, out


def scatter(name):
    pts = [  # (label, cost per task in USD, judge, tazuna?, label side)  README 0.0.11 table, 2026-10-07
        ("tazuna", 0.41, 6.67, True, "right"), ("OMP", 0.51, 6.54, False, "right"),
        ("Claude Code", 0.43, 6.38, False, "right"), ("Hermes", 0.55, 6.21, False, "right"),
        ("pi", 0.28, 6.08, False, "left"), ("tazuna lean", 0.31, 6.04, True, "right"),
        ("OMO", 0.49, 5.38, False, "right"), ("Empryo", 0.50, 5.12, False, "right"),
    ]
    sub = ["Six merged Go PRs, two runs each, hidden tests and a blind Opus judge; every harness on Claude Sonnet 5.5",
           "high (2026-10-07). Up and to the left is better; under 0.75 judge points is within noise."]
    out = header("tazuna against other harnesses", sub, [("tazuna", "hi"), ("others", "lo")])
    left, right, top, bottom = 70, W - 40, 90, 360
    sx = lambda v: left + (right - left) * (v - 0.25) / 0.35
    sy = lambda v: bottom - (bottom - top) * (v - 5.0) / 2.0
    for t in (5.0, 5.5, 6.0, 6.5, 7.0):
        out.append(f'<line x1="{left}" x2="{right}" y1="{sy(t):.1f}" y2="{sy(t):.1f}" stroke="{{grid}}" stroke-width="1"/>')
        out.append(text(left - 8, sy(t) + 4, f"{t:.1f}", 11, "muted", "end"))
    for t in (0.30, 0.40, 0.50, 0.60):
        out.append(text(sx(t), bottom + 18, f"${t:.2f}", 11, "muted", "middle"))
    out.append(f'<line x1="{left}" x2="{right}" y1="{bottom}" y2="{bottom}" stroke="{{axis}}" stroke-width="1"/>')
    out.append(text((left + right) / 2, bottom + 38, "median cost per task", 11, "muted", "middle"))
    out.append(f'<text x="20" y="{(top + bottom) / 2:.1f}" font-size="11" fill="{{muted}}" text-anchor="middle" '
               f'transform="rotate(-90 20 {(top + bottom) / 2:.1f})">judge score (1-10)</text>')
    for label, cost, judge, ours, side in sorted(pts, key=lambda p: p[3]):  # tazuna's points on top
        x, y = sx(cost), sy(judge)
        out.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="6" fill="{{{"hi" if ours else "lo"}}}" stroke="{{ring}}" stroke-width="2"/>')
        dx, anchor = (10, "start") if side == "right" else (-10, "end")
        out.append(text(x + dx, y + 4, label, 12, "ink", anchor, 600 if ours else 400))
    return name, bottom + 52, out


def charts():
    yield scatter("harnesses")
    yield bars(
        "Same model: its maker's harness against tazuna",
        ["Minutes per task, shorter is better; four Go PRs, one run each, blind judge (DeepSeek on Mistral's API:",
         "tazuna 0.1.73 against Vibe CLI 3.1 at max; Kimi on SubRouter: tazuna 0.1.4 against Kimi Code 2.4.0)."],
        [("tazuna's own loop", "hi"), ("the maker's harness", "lo")],
        [("DeepSeek V4.1 Flash", [("Mistral Vibe CLI", 21, "lo", "21 min · judge 7.50 · 10/11 tests"),
                                  ("tazuna", 7, "hi", "7 min · judge 8.00 · 11/11 tests")]),
         ("Kimi K3.1 Pro", [("Kimi Code", 20.5, "lo", "20.5 min · judge 7.25 · 11/13 tests"),
                            ("tazuna", 14.25, "hi", "14.3 min · judge 6.88 · 11/13 tests")])],
        25, (0, 5, 10, 15, 20, 25), lambda t: f"{t} min", "same-model")
    yield bars(
        "What a task costs",
        ["Cost per task. Opus rows: six Go PRs, judge within 0.25 of stock Claude Code (ROADMAP, 2026-10). DeepSeek row:",
         "14 PRs merged in October, after DeepSeek's release; the cheap model solved more of them (13 against 9)."],
        [("tazuna", "hi"), ("reference", "lo")],
        [("Claude Opus 5.5, six Go PRs", [("Claude Code", 0.91, "lo", "$0.91 · judge 7.58"),
                                          ("tazuna", 0.66, "hi", "$0.66 · judge 7.46"),
                                          ("tazuna, boss mode", 0.45, "hi", "$0.45 · judge 7.33")]),
         ("Code that may leave the machine, 14 October PRs", [("tazuna on Opus 5.5", 2.78, "lo", "$2.78 · judge 6.79 · 9 of 14 solved"),
                                                              ("tazuna on DeepSeek V4.1", 0.60, "hi", "$0.60 · judge 7.79 · 13 of 14 solved")])],
        4, (0, 1, 2, 3, 4), lambda t: f"${t:g}", "cost")
    yield bars(
        "Memory",
        ["Peak RAM on an M5 Pro. UI: tazuna 0.1.83 (bench/footprint.py) against Claude Code's own UI (2026-10-08).",
         "Whole agent run on one small fix: tazuna 0.0.50 (bench/agent_footprint.py)."],
        [("tazuna", "hi"), ("with the claude CLI", "lo")],
        [("The terminal UI", [("Claude Code", 190, "lo", "190 MB"), ("tazuna", 7.8, "hi", "7.8 MB")]),
         ("A whole agent run", [("tazuna on the claude CLI", 282, "lo", "262-282 MB"),
                                ("tazuna's own loop", 22, "hi", "21-22 MB")])],
        300, (0, 50, 100, 150, 200, 250, 300), lambda t: f"{t} MB", "memory")


def main():
    out_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "img")
    os.makedirs(out_dir, exist_ok=True)
    for name, height, parts in charts():
        for theme, colors in THEMES.items():
            body = "\n".join(parts)
            for k, v in colors.items():
                body = body.replace("{" + k + "}", v)
            svg = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {height}" width="{W}" height="{height}" '
                   f'font-family="{FONT}" role="img" aria-label="{escape(name)}">\n{body}\n</svg>\n')
            with open(os.path.join(out_dir, f"{name}-{theme}.svg"), "w") as f:
                f.write(svg)
            print(f"docs/img/{name}-{theme}.svg")


if __name__ == "__main__":
    main()
