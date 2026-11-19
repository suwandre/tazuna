// In the page before the app (an init script, or a <script> in a built copy): Tauri's internals over review/bridge.mjs,
// so the window runs on the real core. `window.TAZUNA_BRIDGE` picks another bridge than http://127.0.0.1:4720.
(() => {
  if (window.__TAURI_INTERNALS__) return;
  const BASE = window.TAZUNA_BRIDGE || "http://127.0.0.1:4720";
  // a Linux agent: on a Mac one the app leaves its shortcuts to the native menus, which a browser does not have
  Object.defineProperty(navigator, "userAgent", { get: () => "Mozilla/5.0 (X11; Linux x86_64) Chrome/140" });
  localStorage.setItem("tazuna.setup", "1");
  const h = {}, cbs = {}; let n = 0;
  const es = new EventSource(`${BASE}/events`);
  es.onmessage = (e) => { const ev = JSON.parse(e.data); for (const id of h[ev.event] || []) cbs[id]({ event: ev.event, id: 1, payload: ev.payload }); };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  window.__TAURI_INTERNALS__ = { metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
    transformCallback(cb) { const id = ++n; cbs[id] = cb; return id; },
    async invoke(cmd, args) {
      if (cmd === "plugin:event|listen") { (h[args.event] ??= []).push(args.handler); return n; }
      const r = await (await fetch(`${BASE}/`, { method: "POST", body: JSON.stringify({ cmd, args }) })).json();
      if (r.err) throw r.err;
      return r.ok;
    } };
})();
