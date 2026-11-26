// The window's own manners, set up once (main.tsx): tooltips in its style, and a way out for what closes.
import { useSyncExternalStore } from "react";

/** A control's `title` shown in the window's tooltip (`.tz-tip`), not the system's: that one came after a second, in the
 * system's font and colours, and there were 118 of them (design review 8). The title is taken off while the pointer is on
 * the control (the system shows nothing) and put back when it leaves, so the name is there for everything else. */
export function installTooltips() {
  const tip = document.createElement("div");
  tip.className = "tz-tip";
  tip.setAttribute("role", "tooltip");
  document.body.appendChild(tip);
  let at: HTMLElement | null = null, text = "", timer = 0, named = false, said = false, pressed = false, quiet = false, armed = false;
  const hide = () => {
    clearTimeout(timer);
    tip.classList.remove("on");
    // back only if nothing set a new one meanwhile (React, when the title changed)
    if (at?.isConnected && !at.hasAttribute("title")) at.setAttribute("title", text);
    if (named) at?.removeAttribute("aria-label");
    if (said) at?.removeAttribute("aria-description");
    at = null; named = false; said = false;
  };
  // a row marked `data-tip-clip` says in its title only what it shows: the tooltip comes when some of it is cut
  // (the part its title repeats is marked `data-tip-text`; without one, any cut part counts)
  const cut = (el: HTMLElement) => { const parts = el.querySelectorAll<HTMLElement>("[data-tip-text]"); return (parts.length ? [...parts] : [el, ...el.querySelectorAll<HTMLElement>(".truncate")]).some((e) => e.scrollWidth > e.clientWidth + 1); };
  const soon = (el: HTMLElement, wait: number, byPointer: boolean) => { clearTimeout(timer); timer = window.setTimeout(() => at === el && !(byPointer && pressed) && show(el), wait); };
  const show = (el: HTMLElement) => {
    tip.textContent = text;
    const r = el.getBoundingClientRect(), t = tip.getBoundingClientRect(), gap = 6;
    const below = r.bottom + gap + t.height <= window.innerHeight - 4;
    tip.style.left = `${Math.round(Math.max(6, Math.min(r.left + r.width / 2 - t.width / 2, window.innerWidth - t.width - 6)))}px`;
    tip.style.top = `${Math.round(below ? r.bottom + gap : r.top - gap - t.height)}px`;
    tip.classList.add("on");
  };
  const over = (target: EventTarget | null, wait: number, byPointer: boolean) => {
    const el = (target as Element | null)?.closest?.<HTMLElement>("[title]") ?? null;
    if (el === at || (at && !el && at.contains(target as Node))) return;
    hide();
    const title = el?.getAttribute("title");
    if (!el || !title) return;
    at = el; text = title;
    el.removeAttribute("title");
    // a control with an icon alone has only its title for a name: it keeps one while the title is off (a screen reader
    // on the focused send button read nothing, design review 11); one with words of its own keeps the title as its
    // description
    if (!el.textContent?.trim()) { if (!el.hasAttribute("aria-label") && !el.hasAttribute("aria-labelledby")) { el.setAttribute("aria-label", title); named = true; } }
    else if (!el.hasAttribute("aria-description")) { el.setAttribute("aria-description", title); said = true; }
    quiet = el.dataset.tipClip !== undefined && !cut(el);
    // by the pointer, the wait starts at its next real move (below): an element that moved under a still pointer (rows
    // after Ctrl+N or a press) gets no tooltip (design review 12: one came up on a sidebar row that slid under it)
    armed = false;
    if (!quiet && !byPointer) soon(el, wait, false);
  };
  document.addEventListener("pointerover", (e) => over(e.target, 450, true));
  // the keyboard's way to the same words
  document.addEventListener("focusin", (e) => { if ((e.target as Element).matches?.(":focus-visible")) over(e.target, 450, false); });
  for (const ev of ["pointerdown", "keydown", "focusout", "wheel"]) document.addEventListener(ev, hide, true);
  // after a press the rows under the pointer may move: no tooltip over them until the pointer itself moves 3 px or more
  let px = 0, py = 0, mx = -1, my = -1;
  document.addEventListener("pointerdown", (e) => { pressed = true; px = e.clientX; py = e.clientY; }, true);
  document.addEventListener("pointermove", (e) => {
    // the browser's own "move" when the page moves under a still pointer has the pointer where it was: not a move
    const moved = e.clientX !== mx || e.clientY !== my;
    mx = e.clientX; my = e.clientY;
    if (pressed && Math.abs(e.clientX - px) + Math.abs(e.clientY - py) > 3) { pressed = false; armed = false; }
    if (!moved || pressed || armed || !at || quiet || tip.classList.contains("on")) return;
    armed = true;
    soon(at, 450, true);
  }, true);
  document.addEventListener("keydown", () => { pressed = false; }, true);
  document.addEventListener("pointerleave", hide);
}

/** A closed menu or dialog fades out (`.tz-out`) where it was: entries had their motion since desktop 0.1.74, exits were
 * a cut. React takes the element out at once, as before; a copy that takes no input stays for the length of the fade. One
 * place for every menu and dialog, however it was closed. */
export function installExits() {
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  const root = document.getElementById("root");
  if (!root) return;
  new MutationObserver((records) => {
    for (const r of records) for (const n of r.removedNodes) {
      // still there: React's development mode takes a new element out and puts it back once
      if (!(n instanceof HTMLElement) || n.isConnected || !r.target.isConnected || !n.matches(".tz-fade, .tz-pop") || n.classList.contains("tz-out")) continue;
      const ghost = n.cloneNode(true) as HTMLElement;
      ghost.classList.add("tz-out");
      ghost.inert = true;
      ghost.setAttribute("aria-hidden", "true");
      r.target.insertBefore(ghost, r.nextSibling?.parentNode === r.target ? r.nextSibling : null);
      setTimeout(() => ghost.remove(), 130);
    }
  }).observe(root, { childList: true, subtree: true });
}

/** How the window looks, chosen in Settings and kept on this machine: light or dark (or the system's), the accent, the text
 * size (the webview's own zoom, as a browser's), the type answers are set in, the work panel's Stats as tiles or rows, and
 * where the conversation's numbers stand (under the box, the default; above it; in the status bar; off), and whether they and
 * the Stats count live while a reply streams (on, the default; "off": as each model call ends). */
export const THEME = "tazuna.theme", ACCENT = "tazuna.accent", ZOOM = "tazuna.zoom", PROSE = "tazuna.prose", STATS = "tazuna.stats", NUMBERS = "tazuna.numbers", LIVE = "tazuna.live-stats";
/** The accents (design review 13, D2 of mockups/v10.html): Ai, styles.css's own tokens (the default since design review 14:
 * nothing kept, or none of these, is Ai; one picked by hand stays), or one drawn for both themes in styles.css
 * (`[data-accent=…]`). */
export const ACCENTS = ["Ai", "Indigo", "Jade", "Amber", "Rose", "Graphite"];
export const accentNow = () => { const a = localStorage.getItem(ACCENT) ?? ""; return ACCENTS.includes(a) ? a : "Ai"; };
export const TEXT_SIZES: [string, string][] = [["0.9", "Small"], ["1", "Default"], ["1.1", "Large"], ["1.25", "Larger"]];

/** One of them changed: kept, and applied now (`tz-looks` for what reads one as it renders: `useLook`). */
export function look(key: string, value: string) {
  localStorage.setItem(key, value);
  applyLooks();
  window.dispatchEvent(new Event("tz-looks"));
}
const onLooks = (f: () => void) => { window.addEventListener("tz-looks", f); return () => window.removeEventListener("tz-looks", f); };
/** A choice read as it renders, and again when one changes (the Stats' layout, where the numbers stand). */
export const useLook = (key: string) => useSyncExternalStore(onLooks, () => localStorage.getItem(key));

/** Dark now, from the choice and the system. */
export const darkNow = () => { const t = localStorage.getItem(THEME); return t === "dark" || (t !== "light" && window.matchMedia("(prefers-color-scheme: dark)").matches); };

export function applyLooks() {
  // how wide this machine's scrollbars are (0 where they lie over the page, as on a Mac): what sits on a scroller's right
  // edge (the transcript's fades, the work panel's tab) stays clear of them
  if (!document.documentElement.style.getPropertyValue("--tz-sbw")) {
    const probe = document.createElement("div");
    probe.style.cssText = "position:absolute;top:-99px;width:50px;height:50px;overflow:scroll";
    document.body.appendChild(probe);
    document.documentElement.style.setProperty("--tz-sbw", `${probe.offsetWidth - probe.clientWidth}px`);
    probe.remove();
  }
  document.documentElement.classList.toggle("dark", darkNow());
  // the accent as an attribute styles.css draws (design review 13): the root's inline style is a plugin's (theme.set), and
  // the accent set there wiped a plugin's own on every flip of the theme
  const accent = accentNow();
  if (accent !== "Ai") document.documentElement.dataset.accent = accent.toLowerCase();
  else delete document.documentElement.dataset.accent;
  document.documentElement.classList.toggle("prose-sans", localStorage.getItem(PROSE) === "sans");
  const zoom = Number(localStorage.getItem(ZOOM)) || 1;
  // only inside the app's window: a browser has its own zoom. The window's top row divides by it (`--tz-pt`, styles.css)
  if ("__TAURI_INTERNALS__" in window) {
    document.documentElement.style.setProperty("--tz-z", String(zoom));
    import("@tauri-apps/api/webview").then(({ getCurrentWebview }) => getCurrentWebview().setZoom(zoom)).catch(() => {});
  }
}
