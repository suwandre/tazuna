import { useCallback, useRef, useState } from "react";

export const FOCUSABLE = 'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])';

/** A modal panel's keyboard behaviour (accessibility pass, 0.1.34): focus moves into it when it opens (unless something
 * inside took it already, such as a search box), Tab and Shift-Tab stay inside it, and focus goes back where it was when
 * it closes; focus taken behind it comes back. The panel's ref, beside role="dialog", aria-modal, a label and tabIndex -1; each panel keeps its own Escape. */
export function useDialog() {
  const cleanup = useRef<(() => void) | undefined>(undefined);
  // where focus was when the panel opened: read at its first render, before a search box inside takes focus
  const [before] = useState(() => document.activeElement as HTMLElement | null);
  return useCallback((el: HTMLElement | null) => {
    cleanup.current?.();
    cleanup.current = undefined;
    if (!el) return;
    if (!el.contains(document.activeElement)) el.focus();
    const trap = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;
      const items = [...el.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((x) => !x.hasAttribute("disabled") && x.offsetParent !== null);
      if (!items.length) { e.preventDefault(); return; }
      const first = items[0], last = items[items.length - 1], at = document.activeElement;
      if (e.shiftKey && (at === first || at === el)) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && at === last) { e.preventDefault(); first.focus(); }
    };
    el.addEventListener("keydown", trap);
    // focus taken behind the panel (a conversation opening there focuses its message box) comes back; another dialog over
    // this one keeps it
    const back = (e: FocusEvent) => { if (!el.contains(e.target as Node) && !(e.target as HTMLElement).closest?.("[role=dialog], [role=alertdialog]")) el.focus(); };
    document.addEventListener("focusin", back);
    // focus goes back once the panel is really gone (React's development mode detaches and attaches it once on opening)
    // and only when nothing else took it (an action that moved on, such as New chat into the composer, keeps its focus)
    cleanup.current = () => {
      el.removeEventListener("keydown", trap);
      document.removeEventListener("focusin", back);
      setTimeout(() => { if (!el.isConnected && before?.isConnected && (!document.activeElement || document.activeElement === document.body)) before.focus(); });
    };
  }, [before]);
}
