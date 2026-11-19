// The window's own manners, set up once (main.tsx): tooltips in its style, and a way out for what closes.

/** A control's `title` shown in the window's tooltip (`.tz-tip`), not the system's: that one came after a second, in the
 * system's font and colours, and there were 118 of them (design review 8). The title is taken off while the pointer is on
 * the control (the system shows nothing) and put back when it leaves, so the name is there for everything else. */
export function installTooltips() {
  const tip = document.createElement("div");
  tip.className = "tz-tip";
  tip.setAttribute("role", "tooltip");
  document.body.appendChild(tip);
  let at: HTMLElement | null = null, text = "", timer = 0;
  const hide = () => {
    clearTimeout(timer);
    tip.classList.remove("on");
    // back only if nothing set a new one meanwhile (React, when the title changed)
    if (at?.isConnected && !at.hasAttribute("title")) at.setAttribute("title", text);
    at = null;
  };
  const show = (el: HTMLElement) => {
    tip.textContent = text;
    const r = el.getBoundingClientRect(), t = tip.getBoundingClientRect(), gap = 6;
    const below = r.bottom + gap + t.height <= window.innerHeight - 4;
    tip.style.left = `${Math.round(Math.max(6, Math.min(r.left + r.width / 2 - t.width / 2, window.innerWidth - t.width - 6)))}px`;
    tip.style.top = `${Math.round(below ? r.bottom + gap : r.top - gap - t.height)}px`;
    tip.classList.add("on");
  };
  const over = (target: EventTarget | null, wait: number) => {
    const el = (target as Element | null)?.closest?.<HTMLElement>("[title]") ?? null;
    if (el === at || (at && !el && at.contains(target as Node))) return;
    hide();
    const title = el?.getAttribute("title");
    if (!el || !title) return;
    at = el; text = title;
    el.removeAttribute("title");
    timer = window.setTimeout(() => at === el && show(el), wait);
  };
  document.addEventListener("pointerover", (e) => over(e.target, 450));
  // the keyboard's way to the same words
  document.addEventListener("focusin", (e) => { if ((e.target as Element).matches?.(":focus-visible")) over(e.target, 450); });
  for (const ev of ["pointerdown", "keydown", "focusout", "wheel"]) document.addEventListener(ev, hide, true);
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

/** How the window looks, chosen in Settings and kept on this machine: light or dark (or the system's), the text size (the
 * webview's own zoom, as a browser's), and the type answers are set in. */
export const THEME = "tazuna.theme", ZOOM = "tazuna.zoom", PROSE = "tazuna.prose";
export const TEXT_SIZES: [string, string][] = [["0.9", "Small"], ["1", "Default"], ["1.1", "Large"], ["1.25", "Larger"]];

/** One of them changed: kept, and applied now. */
export function look(key: string, value: string) {
  localStorage.setItem(key, value);
  applyLooks();
}

/** Dark now, from the choice and the system. */
export const darkNow = () => { const t = localStorage.getItem(THEME); return t === "dark" || (t !== "light" && window.matchMedia("(prefers-color-scheme: dark)").matches); };

export function applyLooks() {
  document.documentElement.classList.toggle("dark", darkNow());
  document.documentElement.classList.toggle("prose-sans", localStorage.getItem(PROSE) === "sans");
  const zoom = Number(localStorage.getItem(ZOOM)) || 1;
  // only inside the app's window: a browser has its own zoom
  if ("__TAURI_INTERNALS__" in window) import("@tauri-apps/api/webview").then(({ getCurrentWebview }) => getCurrentWebview().setZoom(zoom)).catch(() => {});
}
