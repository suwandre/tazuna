// tazuna's console reader: the initialization script of a provider's sign-in window (main.rs, console_window, which calls
// this function with the window, {provider, mode: "learn" | "read", sites} and the provider's extractor or null). It acts
// only over https on that provider's own sites, and makes no request of its own: it wraps fetch and XMLHttpRequest to look
// at the JSON the page itself gets from those sites. Learn mode reports each response's shape (field names and value
// types; no values, cookies, headers or tokens) and, where usage-like fields appear, the page they appeared on; read mode
// runs the extractor on usage-like JSON and reports its numbers. Everything goes through one command,
// plugin:console|report, which checks it all again. It draws nothing on the page (a bar over it hid Mistral's own sign-in):
// what learning found is the window's title (main.rs learn_title). review/console.cjs checks the pure parts and the wrappers.
(function (W, CFG, READ) {
  "use strict";
  // no "token": accessToken is a sign-in, not usage
  var USAGE = /quota|usage|remain|balance|credit|limit|used|total|plan/i;
  // a field name or path part that may be a value becomes "*" or ":id" (main.rs value_like has the same rules): a UUID, five
  // digits or sixteen hex digits in a row, a run of 8+ letters and digits mixed, or of 8+ with a third of it capitals
  // (sk-AbCdEfGh), a prefixed id (cus_NffrFeUf, key_2aBc…); a field name with a dot, a dash or an @ (john.smith, acme-prod);
  // a path part with a dot unless it is a file's name (api.json)
  var NAME = /^[A-Za-z_$][\w$]{0,63}$/, PART = /^[A-Za-z][\w-]{0,63}$|^[A-Za-z][A-Za-z-]{0,40}\.(json|php|html?|aspx?|do)$/;
  var UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i, PREFIXED = /(^|[^A-Za-z0-9])[a-z]{2,6}[_-](?=[A-Za-z0-9]*[A-Z0-9])[A-Za-z0-9]{6,}/;
  function valueLike(s) {
    return UUID.test(s) || /\d{5,}|[0-9a-f]{16,}/i.test(s) || PREFIXED.test(s) || s.split(/[^A-Za-z0-9]+/).some(function (t) {
      var caps = t.replace(/[^A-Z]/g, "").length;
      return t.length >= 8 && ((/\d/.test(t) && /[A-Za-z]/.test(t)) || (/[a-z]/.test(t) && caps >= 2 && caps * 3 >= t.length));
    });
  }
  // query keys a learned page keeps, with the values each may have (main.rs SAFE_QUERY, the same): the region a console
  // shows its usage for, so a reading opens the right one
  var SAFE_QUERY = { region: ["ap-southeast-1", "us-east-1", "cn-beijing", "cn-hongkong", "eu-central-1"] };
  function key(k) { return NAME.test(k) && !valueLike(k) ? k : "*"; }
  function part(s) { return PART.test(s) && !valueLike(s) ? s : ":id"; }
  /** A URL's path with its parts made safe, and its query's keys alone: "/api/users/:id/usage?region&lang". */
  function cleanPath(u, query) {
    var q = [];
    if (query) u.searchParams.forEach(function (_, k) { k = key(k); if (q.indexOf(k) < 0) q.push(k); });
    var p = u.pathname.split("/").map(function (s, i) { return i && s ? part(s) : s; }).join("/").slice(0, 300);
    return p + (q.length ? "?" + q.slice(0, 20).join("&") : "");
  }
  /** The page a usage response came on: its cleaned path and its region where the list has it ("/usage?region=ap-southeast-1"). */
  function pageOf(u) {
    var q = [];
    u.searchParams.forEach(function (v, k) {
      if (Object.prototype.hasOwnProperty.call(SAFE_QUERY, k) && SAFE_QUERY[k].indexOf(v) >= 0 && !q.some(function (x) { return x.split("=")[0] === k; })) q.push(k + "=" + v);
    });
    return u.origin + cleanPath(u, false) + (q.length ? "?" + q.join("&") : "");
  }
  /** A value's shape: objects keep their keys, arrays their first item, every other value its type's name. */
  function shape(v, d) {
    d = d || 0;
    if (v === null) return "null";
    if (Array.isArray(v)) return d >= 10 || !v.length ? [] : [shape(v[0], d + 1)];
    if (typeof v === "object") {
      var o = {}, n = 0;
      if (d >= 10) return o;
      for (var k in v) {
        if (!Object.prototype.hasOwnProperty.call(v, k)) continue;
        var c = key(k);
        if (Object.prototype.hasOwnProperty.call(o, c)) continue;
        if (++n > 100) break;
        o[c] = shape(v[k], d + 1);
      }
      // a map (its keys are names of things: customers, projects) whose children share one shape: its keys go
      var kids = Object.keys(o).map(function (k) { return o[k]; });
      if (kids.length >= 2 && kids.every(function (x) { return x && typeof x === "object" && JSON.stringify(x) === JSON.stringify(kids[0]); })) return { "*": kids[0] };
      return o;
    }
    return typeof v === "number" ? "number" : typeof v === "boolean" ? "boolean" : "string";
  }
  /** The usage-like field names in a shape. */
  function usageKeys(s, out) {
    out = out || [];
    if (Array.isArray(s)) s.forEach(function (x) { usageKeys(x, out); });
    else if (s && typeof s === "object") for (var k in s) { if (USAGE.test(k) && out.indexOf(k) < 0) out.push(k); usageKeys(s[k], out); }
    return out;
  }
  function onSite(sites, host) {
    return sites.indexOf(host) >= 0;
  }
  var api = { valueLike: valueLike, key: key, part: part, cleanPath: cleanPath, pageOf: pageOf, shape: shape, usageKeys: usageKeys, onSite: onSite };
  if (!W || !CFG || W.location.protocol !== "https:" || !onSite(CFG.sites, W.location.hostname)) return api;

  var noop = function () {}, sent = [];
  function report(kind, payload) {
    var I = W.__TAURI_INTERNALS__;
    if (!I) return;
    var body = JSON.stringify(payload);
    if (body && body.length > 60000) return;
    // the same shape of the same call once per page
    if (kind === "learn") { if (sent.indexOf(body) >= 0 || sent.length > 500) return; sent.push(body); }
    Promise.resolve(I.invoke("plugin:console|report", { provider: CFG.provider, kind: kind, payload: payload })).catch(noop);
  }
  /** A response the page got: kept only from the provider's sites, over https, with a JSON type. */
  function site(url) {
    try { var u = new URL(url, W.location.href); return u.protocol === "https:" && onSite(CFG.sites, u.hostname) ? u : null; } catch (e) { return null; }
  }
  function seen(u, method, status, type, body) {
    if (!/json/i.test(type || "") || typeof body !== "string" || body.length > 2e6) return;
    var j;
    try { j = JSON.parse(body); } catch (e) { return; }
    if (!j || typeof j !== "object") return;
    var s = shape(j), use = usageKeys(s);
    if (CFG.mode === "learn") {
      var r = { path: cleanPath(u, true), method: String(method || "GET").toUpperCase().slice(0, 8), status: status | 0, shape: s };
      if (use.length) r.page = pageOf(new URL(W.location.href));
      report("learn", r);
    } else if (use.length && typeof READ === "function") {
      var n = null;
      try { n = READ(cleanPath(u, false), j); } catch (e) { n = null; }
      if (n) report("read", { numbers: n });
    }
  }
  var fetch0 = W.fetch;
  if (typeof fetch0 === "function") {
    W.fetch = function (input, init) {
      var p = fetch0.apply(this, arguments);
      try {
        var method = (init && init.method) || (input && typeof input === "object" && input.method) || "GET";
        var asked = typeof input === "string" ? input : input && (input.url || String(input));
        p.then(function (r) {
          var u = site(r.url || asked), type = r.headers && r.headers.get("content-type");
          if (u && /json/i.test(type || "")) r.clone().text().then(function (t) { seen(u, method, r.status, type, t); }, noop);
        }, noop);
      } catch (e) { /* never in the page's way */ }
      return p;
    };
  }
  var X = W.XMLHttpRequest && W.XMLHttpRequest.prototype;
  if (X) {
    var open0 = X.open, send0 = X.send, calls = new WeakMap();
    X.open = function (method, url) { calls.set(this, [method, url]); return open0.apply(this, arguments); };
    X.send = function () {
      var x = this, a = calls.get(x);
      if (a) x.addEventListener("load", function () {
        try {
          var u = site(x.responseURL || a[1]), type = x.getResponseHeader("content-type");
          if (!u) return;
          var t = x.responseType === "" || x.responseType === "text" ? x.responseText : x.responseType === "json" ? JSON.stringify(x.response) : null;
          seen(u, a[0], x.status, type, t);
        } catch (e) { /* never in the page's way */ }
      });
      return send0.apply(this, arguments);
    };
  }
  // a sign-in page where the usage page was asked for (read mode): signed out
  if (CFG.mode === "read" && /(^|\/)(log-?in|sign-?in|auth)(\/|$)/i.test(W.location.pathname)) report("signed_out", null);
  return api;
})
