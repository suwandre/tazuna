// In-page checks (page.evaluate(require("./audit.cjs"))), run on each screen: text cut without an ellipsis, contrast under WCAG AA, click targets under 24 px,
// anything wider than the window
module.exports = () => {
  const vis = (e) => { const r = e.getBoundingClientRect(); const s = getComputedStyle(e); return r.width > 0 && r.height > 0 && s.visibility !== "hidden" && s.opacity !== "0"; };
  const rgb = (c) => { const m = c.match(/[\d.]+/g); return m ? m.map(Number) : null; };
  const lum = ([r, g, b]) => { const f = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }; return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b); };
  const bgOf = (e) => { for (let n = e; n; n = n.parentElement) { const c = rgb(getComputedStyle(n).backgroundColor); if (c && (c[3] === undefined || c[3] > 0.5)) return c; } return [255, 255, 255]; };
  const out = { clipped: [], contrast: {}, small: [], overflowX: document.documentElement.scrollWidth > innerWidth + 1 };
  for (const e of document.querySelectorAll("body *")) {
    if (!vis(e)) continue;
    const s = getComputedStyle(e);
    const own = [...e.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim());
    if (own && e.scrollWidth > e.clientWidth + 1 && s.overflowX !== "visible" && s.textOverflow !== "ellipsis" && !/auto|scroll/.test(s.overflowX))
      out.clipped.push(`${e.tagName.toLowerCase()} "${e.textContent.trim().slice(0, 50)}"`);
    if (own) {
      const fg = rgb(s.color), bg = bgOf(e); if (!fg) continue;
      const a = fg[3] ?? 1, mix = fg.slice(0, 3).map((v, i) => v * a + bg[i] * (1 - a));
      const L1 = lum(mix), L2 = lum(bg), ratio = (Math.max(L1, L2) + 0.05) / (Math.min(L1, L2) + 0.05);
      const size = parseFloat(s.fontSize), large = size >= 18 || (size >= 14 && Number(s.fontWeight) >= 700);
      if (ratio < (large ? 3 : 4.5)) { const k = `${s.color} on rgb(${bg.slice(0, 3).join(",")}) ${ratio.toFixed(2)}`; (out.contrast[k] ??= []).push(e.textContent.trim().slice(0, 30)); }
    }
    if (e.matches("button, a, [role=button], input[type=checkbox], [role=radio], [role=switch]")) {
      const r = e.getBoundingClientRect(); if (r.width < 24 || r.height < 20) out.small.push(`${e.tagName.toLowerCase()} ${Math.round(r.width)}x${Math.round(r.height)} "${(e.getAttribute("aria-label") || e.title || e.textContent).trim().slice(0, 30)}"`);
    }
  }
  out.clipped = [...new Set(out.clipped)].slice(0, 12); out.small = [...new Set(out.small)].slice(0, 15);
  for (const k of Object.keys(out.contrast)) out.contrast[k] = [...new Set(out.contrast[k])].slice(0, 4);
  return out;
};
