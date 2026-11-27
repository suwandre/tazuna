// tazuna's console reader: the initialization script of a provider's sign-in window (main.rs, console_window, which calls
// this function with the window, {provider, mode: "learn" | "read", sites, start} and the provider's extractors or null: JSON's and
// visible text's). It acts only over https on that provider's own sites, and makes no request of its own: it wraps fetch
// and XMLHttpRequest to look at the JSON the page itself gets from those sites, and reads the page's visible text once it
// settles (a page that renders its usage on the server fetches none). Learn mode reports each response's shape (field
// names and value types; no values, cookies, headers or tokens) and, where usage-like fields appear, the page they appeared
// on, and its start page's short usage-like lines with every digit masked ("€##.## of €### used"); read mode runs the extractors on
// usage-like JSON and on the page's text and reports their numbers. Everything goes through one command,
// plugin:console|report, which checks it all again. It draws nothing on the page (a bar over it hid Mistral's own sign-in):
// what learning found is the window's title (main.rs learn_title). review/console.cjs checks the pure parts and the wrappers.
(function (W, CFG, READ, READ_TEXT) {
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
  // a line of the page's text worth learning: a number (any script's digits) and a usage-like word or sign (main.rs
  // TEXT_WORDS, the same); no "of", which any sentence has
  var WORDS = /\b(usage|used|left|remaining|limits?|quotas?|credits?|included|plans?|resets?|renews?|renewal|billing|tokens?|requests?)\b|[%€$]/i;
  /** The page's short usage-like lines, every digit "#" (main.rs text_ok checks it again): a label and its number, 8 words at
   * most (prose, an address, a conversation is longer), none with an @ or a value (valueLike on the masked line: a long
   * count stays; a run of 8+ letters and digits mixed as written, a key's, drops it), 40 lines of 160 at most. */
  function textLines(text) {
    var out = [];
    String(text || "").split("\n").forEach(function (l) {
      l = l.replace(/\s+/g, " ").trim();
      if (out.length >= 40 || l.length > 160 || l.split(" ").length > 8 || !/\p{N}/u.test(l) || !WORDS.test(l) || /[@\uFF20]/.test(l)) return;
      var m = l.replace(/\p{N}/gu, "#");
      if (valueLike(m) || l.split(/[^A-Za-z0-9]+/).some(function (t) { return t.length >= 8 && /\d/.test(t) && /[A-Za-z]/.test(t); })) return;
      if (out.indexOf(m) < 0) out.push(m);
    });
    return out;
  }
  // the parser a text extractor gets as its second argument: (text, p) => {left, total, unit, used_pct, resets} | null
  var NUM = /\d(?:[\d.,]*\d)?/g, MONTHS = "jan feb mar apr may jun jul aug sep oct nov dec".split(" ");
  var MON = "(jan(?:uary)?|feb(?:ruary)?|mar(?:ch)?|apr(?:il)?|may|june?|july?|aug(?:ust)?|sep(?:t(?:ember)?)?|oct(?:ober)?|nov(?:ember)?|dec(?:ember)?)\\b\\.?";
  var DAY = "(\\d{1,2})(?:st|nd|rd|th)?(?!\\d)", YEAR = "(?:,?\\s+(\\d{4}))?";
  var MD = new RegExp("\\b" + MON + "\\s+" + DAY + YEAR, "i"), DM = new RegExp("\\b" + DAY + "\\s+(?:of\\s+)?" + MON + YEAR, "i");
  var parse = {
    /** The first number as a page writes it: "€1,234.56", "1.234,56 €", "12,40", "150". Of two separators the last is the
     * decimal one; one kind alone groups thousands where it comes more than once or before exactly three digits ("1,234",
     * "1.234"), else it is decimal ("12,5"). ponytail: "12.400" reads 12400 (a German thousand); a page with three decimals
     * would need its own rule. */
    num: function (s) {
      var t = (String(s == null ? "" : s).match(NUM) || [])[0];
      if (!t) return null;
      var dot = t.lastIndexOf("."), comma = t.lastIndexOf(","), dec = Math.max(dot, comma);
      if (dec >= 0 && (dot < 0 || comma < 0) && (t.split(t[dec]).length > 2 || (t.length - dec === 4 && !/^0[.,]/.test(t)))) dec = -1;
      return parseFloat((dec < 0 ? t : t.slice(0, dec)).replace(/[.,]/g, "") + (dec < 0 ? "" : "." + t.slice(dec + 1)));
    },
    /** Every number in a line, in order: "€12.40 of €150.00 used" is [12.4, 150]. */
    nums: function (s) { return (String(s == null ? "" : s).match(NUM) || []).map(parse.num); },
    /** The share before a "%": "12% used" is 12, "12,5 %" is 12.5; null without one. */
    pct: function (s) { var m = /(\d(?:[\d.,]*\d)?)\s?%/.exec(String(s == null ? "" : s)); return m ? parse.num(m[1]) : null; },
    /** The first line of a text a pattern finds, trimmed, or "". */
    line: function (text, re) {
      var ls = String(text == null ? "" : text).split("\n");
      for (var i = 0; i < ls.length; i++) if (re.test(ls[i])) return ls[i].trim();
      return "";
    },
    /** A date in a line as epoch seconds (local midnight), or null: "Dec 1", "December 1st, 2026", "1 Dec", "1 December 2026",
     * "2026-12-01", "in 3 days" (also hours, minutes). A date without a year is its next one, today included. */
    date: function (s, now) {
      s = String(s == null ? "" : s);
      now = now || Date.now();
      var rel = /\bin\s+(\d+)\s+(day|hour|minute)s?\b/i.exec(s);
      if (rel) return Math.floor(now / 1000) + rel[1] * { day: 86400, hour: 3600, minute: 60 }[rel[2].toLowerCase()];
      var iso = /\b(\d{4})-(\d{2})-(\d{2})\b/.exec(s), md = MD.exec(s), dm = DM.exec(s), y, m, d;
      if (iso) { y = +iso[1]; m = iso[2] - 1; d = +iso[3]; }
      else if (md && (!dm || md.index <= dm.index)) { m = MONTHS.indexOf(md[1].slice(0, 3).toLowerCase()); d = +md[2]; y = md[3] && +md[3]; }
      else if (dm) { d = +dm[1]; m = MONTHS.indexOf(dm[2].slice(0, 3).toLowerCase()); y = dm[3] && +dm[3]; }
      else return null;
      var today = new Date(now);
      today.setHours(0, 0, 0, 0);
      var t = new Date(y || today.getFullYear(), m, d);
      if (!y && t < today) t = new Date(today.getFullYear() + 1, m, d);
      return t.getDate() === d && t.getMonth() === m ? Math.floor(t.getTime() / 1000) : null;
    },
  };
  var api = { valueLike: valueLike, key: key, part: part, cleanPath: cleanPath, pageOf: pageOf, shape: shape, usageKeys: usageKeys, onSite: onSite, textLines: textLines, parse: parse };
  if (!W || !CFG || W.location.protocol !== "https:" || !onSite(CFG.sites, W.location.hostname)) return api;

  var noop = function () {}, sent = [];
  function report(kind, payload) {
    var I = W.__TAURI_INTERNALS__;
    if (!I) return;
    var body = JSON.stringify(payload);
    if (body && body.length > 60000) return;
    // the same shape of the same call, the same lines of the same page, once
    if (kind === "learn" || kind === "text") { if (sent.indexOf(body) >= 0 || sent.length > 500) return; sent.push(body); }
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
  // the page's visible text once it settles: 1.5 s without a change, or 20 s after it loads with changes still coming; again
  // after an address change (a single-page app's navigation; its new page changes the document too). Learning stops there
  // (and learns text on the start page alone: main.rs checks it again); a reading keeps trying until it has its numbers
  // (its window ends at 30 s). Timers and the observer are the window's as it starts, before the page's own script.
  var D = W.document, MO = W.MutationObserver, later = W.setTimeout, cancel = W.clearTimeout, done = false;
  function onStart() {
    try { var s = new URL(CFG.start), u = new URL(W.location.href); return u.hostname === s.hostname && u.pathname === s.pathname; } catch (e) { return false; }
  }
  function settled() {
    var text = D.body ? String(D.body.innerText || "") : "";
    if (CFG.mode === "learn") {
      var lines = onStart() ? textLines(text) : [];
      if (lines.length) report("text", { page: pageOf(new URL(W.location.href)), lines: lines });
    } else if (!done) {
      var n = null;
      try { n = READ_TEXT(text, parse); } catch (e) { n = null; }
      if (n) { done = true; report("read", { numbers: n }); }
    }
  }
  if (D && MO && later && (CFG.mode === "learn" || typeof READ_TEXT === "function")) {
    var href = null, open = false, quiet = null, cap = null;
    var fire = function () { cancel.call(W, quiet); quiet = null; try { settled(); } catch (e) { /* never in the page's way */ } };
    var poke = function () {
      if (done) return;
      if (W.location.href !== href) {
        href = W.location.href;
        open = true;
        cancel.call(W, cap);
        cap = later.call(W, function () { if (CFG.mode === "learn") open = false; if (quiet) fire(); }, 20000);
      }
      if (!open) return;
      cancel.call(W, quiet);
      quiet = later.call(W, fire, 1500);
    };
    try { new MO(poke).observe(D, { childList: true, subtree: true, characterData: true }); } catch (e) { /* never in the page's way */ }
    poke();
  }
  // a sign-in page where the usage page was asked for (read mode): signed out
  if (CFG.mode === "read" && /(^|\/)(log-?in|sign-?in|auth)(\/|$)/i.test(W.location.pathname)) report("signed_out", null);
  return api;
})
