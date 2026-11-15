#!/usr/bin/env python3
"""Five design directions for tazuna desktop (docs/DESKTOP.md), as static HTML of the same main window: sidebar,
transcript, composer, context rail. `python3 build.py` writes a.html ... e.html and index.html. Mockups only."""
from pathlib import Path

BODY = """
<div class="app">
  <aside class="side">
    <div class="brand"><span class="mark">{mark}</span><span>tazuna</span></div>
    <div class="search">{search_icon}<span>Search</span><kbd>⌘K</kbd></div>
    <button class="new">{plus_icon}<span>New chat</span><kbd>⌘N</kbd></button>
    <div class="group">Chats</div>
    <a class="item">Names for a grey cat</a>
    <a class="item">Train ticket refund (DE)</a>
    <div class="group">Projects</div>
    <a class="proj open">{chev}<span>api-gateway</span><span class="badge">1</span></a>
    <a class="item sub active"><span class="dot run"></span>Retry test fails on CI</a>
    <a class="item sub"><span class="dot"></span>Speed up the build</a>
    <a class="item sub"><span class="dot"></span>Review PR #1468</a>
    <a class="proj">{chev_r}<span>billing-ui</span></a>
    <a class="proj">{chev_r}<span>infra</span></a>
    <div class="side-foot"><span class="avatar">S</span><span>Settings</span><span class="plan">Max · 62% left</span></div>
  </aside>
  <main class="main">
    <header class="top">
      <div class="crumb"><span class="muted">api-gateway</span><span class="sep">/</span><span>Retry test fails on CI</span></div>
      <div class="chips"><span class="chip">{bolt} Opus 5.5 · high</span><span class="chip ghost">main · 2 changed</span></div>
    </header>
    <section class="transcript">
      <div class="msg user"><div class="who">You</div><div class="text">Why does <code>retry.test.ts</code> fail on CI but pass locally?</div></div>
      <div class="msg agent">
        <div class="who">tazuna</div>
        <div class="tools">
          <div class="tool">{ok} Read <span class="path">src/client/retry.ts</span><span class="t">0.8 s</span></div>
          <div class="tool">{ok} Searched <span class="path">“maxDelay”</span> · 4 files<span class="t">0.3 s</span></div>
          <div class="tool">{ok} Ran <span class="path">pnpm test retry</span> · 1 failed<span class="t">6.1 s</span></div>
        </div>
        <div class="prose">
          <p>The test uses real timers. CI runners are slower, so the third retry fires after the test's 5 s timeout: the
          delay doubles from 1 s (1, 2, 4 s) and the cap of 30 s never kicks in.</p>
          <p>I switched the test to fake timers and advanced them explicitly. The code itself is fine.</p>
        </div>
        <div class="diff"><span class="file">{file_icon} src/client/retry.test.ts</span><span class="add">+12</span><span class="del">−3</span><span class="btn">View diff</span><span class="btn ghost">Undo</span></div>
        <div class="meta">$0.042 · 18.4k tokens · 94% cached · 31 s</div>
      </div>
      <div class="msg user"><div class="who">You</div><div class="text">thanks! btw how long should I boil an egg for a runny yolk?</div></div>
      <div class="msg agent chat">
        <div class="who">tazuna <span class="routed">{chat_icon} answered as a chat · <u>send to agent</u></span></div>
        <div class="prose"><p>About 6 minutes from boiling water for a large egg, then straight into cold water. 6½ minutes
        gives a jammy yolk; 9 minutes a firm one.</p></div>
        <div class="meta">$0.003 · 0.9k tokens · 4 s</div>
      </div>
    </section>
    <footer class="composer">
      <div class="box">
        <div class="ph">Ask anything, or describe a change…</div>
        <div class="bar">
          <span class="pill">{shield} auto</span><span class="pill">{bolt} Opus 5.5 · high</span><span class="pill ghost">{clip}</span>
          <span class="grow"></span><span class="cost">last $0.003 · thread $0.045</span><span class="send">{arrow}</span>
        </div>
      </div>
    </footer>
  </main>
  <aside class="rail">
    <div class="card"><div class="label">Context</div><div class="ring"><svg viewBox="0 0 36 36"><circle class="bg" cx="18" cy="18" r="15.9"/><circle class="fg" cx="18" cy="18" r="15.9" stroke-dasharray="34 100"/></svg><span>34%</span></div><div class="small">68k of 200k</div></div>
    <div class="card"><div class="label">Plan · 5 hours</div><div class="bar"><i style="width:38%"></i></div><div class="small">38% used · resets 18:40</div>
      <div class="label mt">Plan · week</div><div class="bar"><i style="width:21%"></i></div><div class="small">21% used</div></div>
    <div class="card"><div class="label">This thread</div><div class="big">$0.045</div><div class="small">2 turns · 94% cached · saved $0.31</div></div>
    <div class="card"><div class="label">Today</div><div class="spark">{spark}</div><div class="small">$1.82 · 37 turns</div></div>
  </aside>
</div>
<div class="switch"><a href="index.html">All designs</a><button onclick="document.documentElement.classList.toggle('dark')">Light / dark</button></div>
"""

ICONS = dict(
    search_icon='<svg class="i" viewBox="0 0 24 24"><circle cx="11" cy="11" r="7"/><path d="M20 20l-3.5-3.5"/></svg>',
    plus_icon='<svg class="i" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14"/></svg>',
    chev='<svg class="i s" viewBox="0 0 24 24"><path d="M6 9l6 6 6-6"/></svg>',
    chev_r='<svg class="i s" viewBox="0 0 24 24"><path d="M9 6l6 6-6 6"/></svg>',
    bolt='<svg class="i s" viewBox="0 0 24 24"><path d="M13 3L4 14h7l-1 7 9-11h-7z"/></svg>',
    ok='<svg class="i s ok" viewBox="0 0 24 24"><path d="M5 12l5 5 9-10"/></svg>',
    file_icon='<svg class="i s" viewBox="0 0 24 24"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/></svg>',
    chat_icon='<svg class="i s" viewBox="0 0 24 24"><path d="M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.4A8 8 0 1 1 21 12z"/></svg>',
    shield='<svg class="i s" viewBox="0 0 24 24"><path d="M12 3l8 3v6c0 4.5-3.4 8.3-8 9-4.6-.7-8-4.5-8-9V6z"/></svg>',
    clip='<svg class="i s" viewBox="0 0 24 24"><path d="M21 11l-8.5 8.5a5 5 0 0 1-7-7L14 4a3.3 3.3 0 0 1 4.7 4.7l-8.5 8.5a1.7 1.7 0 0 1-2.4-2.4L15.5 7"/></svg>',
    arrow='<svg class="i" viewBox="0 0 24 24"><path d="M12 19V5M5 12l7-7 7 7"/></svg>',
    spark='<svg viewBox="0 0 120 32" preserveAspectRatio="none"><path class="sp" d="M0 26 L12 22 L24 24 L36 15 L48 18 L60 9 L72 14 L84 11 L96 17 L108 6 L120 10"/></svg>',
)

BASE = """
*{box-sizing:border-box;margin:0;padding:0}
html,body{height:100%}
body{background:var(--bg);color:var(--ink);font:var(--fs)/1.5 var(--ui);-webkit-font-smoothing:antialiased;user-select:none;cursor:default}
.i{width:16px;height:16px;fill:none;stroke:currentColor;stroke-width:var(--stroke,1.5);stroke-linecap:round;stroke-linejoin:round;flex:none}
.i.s{width:14px;height:14px}
.i.ok{color:var(--ok)}
kbd{font:500 11px var(--mono);color:var(--faint);margin-left:auto;padding:1px 5px;border:1px solid var(--line);border-radius:5px}
code,.path{font-family:var(--mono);font-size:.92em}
.app{display:grid;grid-template-columns:var(--side-w,248px) 1fr var(--rail-w,232px);height:100vh}
.side{background:var(--side);border-right:1px solid var(--line);padding:14px 10px;display:flex;flex-direction:column;gap:2px;overflow:hidden}
.brand{display:flex;align-items:center;gap:8px;font-weight:600;padding:2px 8px 12px;letter-spacing:var(--brand-ls,-.01em)}
.mark{display:grid;place-items:center;width:22px;height:22px;border-radius:var(--r-sm);background:var(--accent);color:var(--on-accent);font:700 12px var(--ui)}
.search,.new{display:flex;align-items:center;gap:8px;padding:7px 9px;border-radius:var(--r);color:var(--muted);font:inherit;background:none;border:none;text-align:left}
.search{background:var(--field);border:1px solid var(--line);margin-bottom:4px}
.new{color:var(--ink)}
.new:hover,.item:hover,.proj:hover{background:var(--hover)}
.group{font:var(--group-font);color:var(--faint);padding:14px 9px 4px;letter-spacing:var(--group-ls,0);text-transform:var(--group-tt,none)}
.item,.proj{display:flex;align-items:center;gap:8px;padding:6px 9px;border-radius:var(--r);color:var(--ink2);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.item.sub{padding-left:28px}
.item.active{background:var(--sel);color:var(--ink)}
.proj span:first-of-type{flex:1}
.badge{font:600 10.5px var(--ui);background:var(--accent);color:var(--on-accent);border-radius:9px;padding:0 6px;line-height:17px}
.dot{width:6px;height:6px;border-radius:50%;background:var(--faint);flex:none}
.dot.run{background:var(--accent);box-shadow:0 0 0 3px var(--accent-soft)}
.side-foot{margin-top:auto;display:flex;align-items:center;gap:8px;padding:10px 8px 2px;border-top:1px solid var(--line);color:var(--muted)}
.avatar{width:22px;height:22px;border-radius:50%;display:grid;place-items:center;background:var(--field);border:1px solid var(--line);font-size:11px;color:var(--ink2)}
.plan{margin-left:auto;font-size:11.5px;color:var(--faint)}
.main{display:flex;flex-direction:column;min-width:0;background:var(--main)}
.top{display:flex;align-items:center;justify-content:space-between;padding:12px 28px;border-bottom:1px solid var(--line-soft)}
.crumb{display:flex;gap:8px;align-items:center;font-weight:500}
.muted{color:var(--muted)}.sep{color:var(--faint)}
.chips{display:flex;gap:6px}
.chip{display:inline-flex;align-items:center;gap:6px;padding:4px 10px;border-radius:var(--r-pill);border:1px solid var(--line);background:var(--field);font-size:12px;color:var(--ink2)}
.chip.ghost{background:none}
.transcript{flex:1;overflow:auto;padding:28px 0 12px}
.msg{max-width:var(--read,720px);margin:0 auto 22px;padding:0 28px}
.who{font:var(--who-font);color:var(--faint);margin-bottom:6px;display:flex;align-items:center;gap:10px;letter-spacing:var(--who-ls,0);text-transform:var(--who-tt,none)}
.msg.user .text{background:var(--user-bg);border:1px solid var(--user-line);padding:10px 14px;border-radius:var(--r-lg);color:var(--ink);display:inline-block;max-width:100%}
.tools{display:flex;flex-direction:column;gap:2px;margin:2px 0 10px;padding-left:var(--tools-pad,0);border-left:var(--tools-line,none)}
.tool{display:flex;align-items:center;gap:8px;font:var(--tool-font);color:var(--muted);padding:2px 0}
.tool .path{color:var(--ink2)}
.tool .t{margin-left:auto;color:var(--faint);font-family:var(--mono);font-size:11px}
.prose{font:var(--prose-font);color:var(--ink)}
.prose p{margin:0 0 10px}
.diff{display:flex;align-items:center;gap:10px;border:1px solid var(--line);background:var(--card);border-radius:var(--r-lg);padding:9px 12px;margin-top:4px;font-size:12.5px}
.diff .file{display:flex;align-items:center;gap:7px;font-family:var(--mono);font-size:12px;color:var(--ink2);flex:1}
.add{color:var(--ok);font:600 12px var(--mono)}.del{color:var(--bad);font:600 12px var(--mono)}
.btn{padding:4px 10px;border-radius:var(--r);background:var(--ink);color:var(--bg);font-size:12px;font-weight:500}
.btn.ghost{background:none;color:var(--muted);border:1px solid var(--line)}
.meta{margin-top:8px;font:11.5px var(--mono);color:var(--faint)}
.routed{display:inline-flex;align-items:center;gap:5px;font:400 11.5px var(--ui);color:var(--muted);letter-spacing:0;text-transform:none;padding:1px 8px;border-radius:var(--r-pill);background:var(--accent-soft)}
.routed u{text-decoration:none;color:var(--accent-ink);font-weight:500}
.composer{padding:10px 28px 22px}
.box{max-width:var(--read,720px);margin:0 auto;border:1px solid var(--line-strong);background:var(--card);border-radius:var(--r-xl);padding:12px 12px 8px 16px;box-shadow:var(--shadow)}
.ph{color:var(--faint);padding:2px 0 14px}
.bar{display:flex;align-items:center;gap:6px}
.pill{display:inline-flex;align-items:center;gap:5px;font-size:12px;color:var(--ink2);padding:3px 9px;border-radius:var(--r-pill);background:var(--field);border:1px solid var(--line)}
.pill.ghost{background:none;border-color:transparent;color:var(--muted)}
.grow{flex:1}
.cost{font:11.5px var(--mono);color:var(--faint);margin-right:6px}
.send{width:30px;height:30px;border-radius:var(--r-pill);display:grid;place-items:center;background:var(--accent);color:var(--on-accent)}
.rail{border-left:1px solid var(--line);background:var(--side);padding:16px 14px;display:flex;flex-direction:column;gap:12px}
.card{background:var(--card);border:1px solid var(--line);border-radius:var(--r-lg);padding:12px 14px}
.label{font:var(--group-font);color:var(--faint);letter-spacing:var(--group-ls,0);text-transform:var(--group-tt,none);margin-bottom:8px}
.label.mt{margin-top:12px}
.ring{position:relative;width:64px;height:64px;margin:2px 0 6px}
.ring svg{width:64px;height:64px;transform:rotate(-90deg)}
.ring circle{fill:none;stroke-width:3.2}.ring .bg{stroke:var(--line)}.ring .fg{stroke:var(--accent);stroke-linecap:round}
.ring span{position:absolute;inset:0;display:grid;place-items:center;font:600 14px var(--ui)}
.card .bar{height:6px;border-radius:4px;background:var(--line);overflow:hidden}
.card .bar i{display:block;height:100%;background:var(--accent);border-radius:4px}
.small{font-size:11.5px;color:var(--muted);margin-top:6px}
.big{font:600 22px var(--num-font,var(--ui));letter-spacing:-.02em}
.spark svg{width:100%;height:34px}.sp{fill:none;stroke:var(--accent);stroke-width:1.6}
.switch{position:fixed;right:12px;bottom:12px;display:flex;gap:6px;font:12px system-ui;z-index:9}
.switch a,.switch button{padding:6px 10px;border-radius:8px;border:1px solid #8884;background:#fffd;color:#222;text-decoration:none;cursor:pointer}
"""

DESIGNS = {
 "a": dict(name="Warm Paper", note="Cursor and Anthropic family: cream canvas, warm ink, one indigo accent, hairlines, a serif for prose.",
   fonts="family=Geist:wght@400;500;600&family=Geist+Mono:wght@400;500;600&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600",
   mark="手", css="""
:root{--ui:'Geist',system-ui,sans-serif;--mono:'Geist Mono',ui-monospace,monospace;--fs:13.5px;
--bg:#f6f4ef;--main:#fbfaf7;--side:#f3f0e9;--card:#fffefb;--field:#efebe3;--hover:#ebe7dd;--sel:#e6e1f2;
--ink:#23221e;--ink2:#3f3c35;--muted:#6f6a60;--faint:#9b958a;--line:#e3ded3;--line-soft:#ece8df;--line-strong:#d8d2c5;
--accent:#3d4aa8;--accent-ink:#2f3a8f;--accent-soft:#e7e8f6;--on-accent:#fff;--ok:#3b7d4f;--bad:#b4473a;
--user-bg:#efebe3;--user-line:transparent;--r-sm:6px;--r:7px;--r-lg:10px;--r-xl:14px;--r-pill:999px;
--shadow:0 1px 2px #2321190d,0 4px 14px #23211908;
--group-font:500 11.5px var(--ui);--who-font:500 12px var(--ui);--tool-font:12.5px var(--ui);
--prose-font:400 15.5px/1.62 'Source Serif 4',Georgia,serif;--read:700px}
.dark{--bg:#171613;--main:#1b1a17;--side:#151411;--card:#211f1b;--field:#2a2823;--hover:#2a2823;--sel:#2b2c45;
--ink:#ece8df;--ink2:#d2cdc2;--muted:#a19b8f;--faint:#77726a;--line:#2f2c27;--line-soft:#25231f;--line-strong:#3a3731;
--accent:#8f9bf0;--accent-ink:#aab3f5;--accent-soft:#272a44;--on-accent:#14152a;--ok:#7cc08f;--bad:#e08b7f;--user-bg:#25231f;--shadow:none}
"""),
 "b": dict(name="Midnight Instrument", note="Linear and Raycast family: near-black graded surfaces, dense, small mono labels, one violet accent.",
   fonts="family=Inter+Tight:wght@400;500;600&family=JetBrains+Mono:wght@400;500;600",
   mark="◆", css="""
:root{--ui:'Inter Tight',system-ui,sans-serif;--mono:'JetBrains Mono',ui-monospace,monospace;--fs:13px;
--bg:#08090a;--main:#0b0c0e;--side:#0e0f12;--card:#131418;--field:#17181d;--hover:#16171c;--sel:#1d1f2a;
--ink:#eceef2;--ink2:#c5c8d0;--muted:#8a8f9c;--faint:#5b606c;--line:#1f2128;--line-soft:#17181d;--line-strong:#2a2c35;
--accent:#7c6cf2;--accent-ink:#a99ff8;--accent-soft:#1f1c3a;--on-accent:#fff;--ok:#4cc38a;--bad:#f2697c;
--user-bg:#141519;--user-line:#22242c;--r-sm:5px;--r:6px;--r-lg:8px;--r-xl:10px;--r-pill:6px;--shadow:0 0 0 1px #ffffff06,0 10px 30px #0006;
--group-font:500 10.5px var(--mono);--group-ls:.06em;--group-tt:uppercase;--who-font:500 10.5px var(--mono);--who-ls:.06em;--who-tt:uppercase;
--tool-font:12px var(--mono);--prose-font:400 14px/1.6 var(--ui);--side-w:232px;--rail-w:220px;--read:760px;--tools-pad:10px;--tools-line:1px solid var(--line)}
.dark{}
html:not(.dark){--bg:#fbfbfc;--main:#fff;--side:#f6f6f8;--card:#fff;--field:#f1f1f4;--hover:#efeff3;--sel:#ebe9fd;
--ink:#16171b;--ink2:#34363d;--muted:#6b6f7a;--faint:#9a9eaa;--line:#e7e7ec;--line-soft:#f0f0f3;--line-strong:#dcdce3;--accent:#5b4ae6;--accent-ink:#4a3ad6;--accent-soft:#eeebfd;--user-bg:#f4f4f7;--user-line:#e7e7ec;--shadow:0 1px 3px #0000000a}
"""),
 "c": dict(name="Quiet Terminal", note="Carries the TUI over: mono-led, monochrome with one status colour, glyph markers, grid alignment.",
   fonts="family=IBM+Plex+Mono:wght@400;500;600&family=IBM+Plex+Sans:wght@400;500;600",
   mark="›", css="""
:root{--ui:'IBM Plex Mono',ui-monospace,monospace;--mono:'IBM Plex Mono',ui-monospace,monospace;--fs:12.5px;
--bg:#f4f4f1;--main:#f7f7f4;--side:#efefeb;--card:#fafaf7;--field:#e9e9e4;--hover:#e6e6e0;--sel:#dfe8dc;
--ink:#1c1d1b;--ink2:#353733;--muted:#6c6e68;--faint:#9a9c95;--line:#dcddd6;--line-soft:#e6e6e0;--line-strong:#cfd0c8;
--accent:#2f7d4f;--accent-ink:#256b42;--accent-soft:#dceadf;--on-accent:#fff;--ok:#2f7d4f;--bad:#b0452f;
--user-bg:transparent;--user-line:transparent;--r-sm:2px;--r:2px;--r-lg:3px;--r-xl:4px;--r-pill:3px;--shadow:none;--stroke:1.4;
--group-font:500 11px var(--mono);--group-tt:lowercase;--who-font:600 11.5px var(--mono);
--tool-font:12px var(--mono);--prose-font:400 13.5px/1.65 'IBM Plex Sans',system-ui,sans-serif;--read:760px;--tools-pad:12px;--tools-line:1px dashed var(--line-strong)}
.msg.user .text{padding:0;border:none}.msg.user .text::before{content:'› ';color:var(--accent);font-weight:600}
.dark{--bg:#121311;--main:#141513;--side:#101110;--card:#181917;--field:#1e201d;--hover:#1e201d;--sel:#1c2a20;
--ink:#e4e5df;--ink2:#c4c6be;--muted:#8e9088;--faint:#62645e;--line:#262824;--line-soft:#1c1e1b;--line-strong:#30322e;--accent:#6ccf8e;--accent-ink:#8fe0aa;--accent-soft:#1a2b1f;--on-accent:#0d1a11;--ok:#6ccf8e;--bad:#ef8a73}
"""),
 "d": dict(name="Editorial Contrast", note="Vercel and Swiss family: pure black and white, strong type scale, hard edges, no accent but ink, generous whitespace.",
   fonts="family=Geist:wght@400;500;600;700&family=Geist+Mono:wght@400;500",
   mark="t", css="""
:root{--ui:'Geist',system-ui,sans-serif;--mono:'Geist Mono',ui-monospace,monospace;--fs:13.5px;
--bg:#fff;--main:#fff;--side:#fafafa;--card:#fff;--field:#f5f5f5;--hover:#f2f2f2;--sel:#ededed;
--ink:#0a0a0a;--ink2:#262626;--muted:#666;--faint:#a3a3a3;--line:#ebebeb;--line-soft:#f2f2f2;--line-strong:#d4d4d4;
--accent:#0a0a0a;--accent-ink:#0a0a0a;--accent-soft:#f0f0f0;--on-accent:#fff;--ok:#0a7d3e;--bad:#c4271a;
--user-bg:#f5f5f5;--user-line:transparent;--r-sm:4px;--r:6px;--r-lg:8px;--r-xl:12px;--r-pill:999px;--shadow:0 2px 8px #0000000a;
--group-font:600 11px var(--ui);--who-font:600 13px var(--ui);--tool-font:12.5px var(--mono);
--prose-font:400 15px/1.65 var(--ui);--read:680px;--brand-ls:-.03em;--num-font:'Geist'}
.brand{font-size:15px;font-weight:700}.crumb{font-weight:600;letter-spacing:-.01em}
.dark{--bg:#0a0a0a;--main:#0a0a0a;--side:#0d0d0d;--card:#111;--field:#171717;--hover:#171717;--sel:#1f1f1f;
--ink:#fafafa;--ink2:#e5e5e5;--muted:#a1a1a1;--faint:#666;--line:#1f1f1f;--line-soft:#161616;--line-strong:#2e2e2e;--accent:#fafafa;--accent-ink:#fafafa;--accent-soft:#1c1c1c;--on-accent:#0a0a0a;--ok:#3ecf7a;--bad:#ff6b5a;--user-bg:#171717;--shadow:none}
"""),
 "e": dict(name="Nordic Calm", note="Soft cool greys and a sage accent, native-feeling rounded controls, airy spacing; calm for long chats.",
   fonts="family=Inter:wght@400;500;600&family=JetBrains+Mono:wght@400;500&family=Newsreader:opsz,wght@6..72,400;6..72,500",
   mark="綱", css="""
:root{--ui:'Inter',system-ui,sans-serif;--mono:'JetBrains Mono',ui-monospace,monospace;--fs:13.5px;
--bg:#eef1f2;--main:#f6f8f8;--side:#e9edee;--card:#fff;--field:#e4e9ea;--hover:#e1e7e8;--sel:#dbe6de;
--ink:#1f2a2e;--ink2:#3a474c;--muted:#66757a;--faint:#94a1a5;--line:#dce3e4;--line-soft:#e6ebec;--line-strong:#d0d9da;
--accent:#4f7d65;--accent-ink:#3f6b54;--accent-soft:#e1ece5;--on-accent:#fff;--ok:#4f7d65;--bad:#b3574a;
--user-bg:#e3eef0;--user-line:transparent;--r-sm:7px;--r:9px;--r-lg:12px;--r-xl:18px;--r-pill:999px;--shadow:0 6px 24px #1f2a2e0f;
--group-font:600 11.5px var(--ui);--who-font:500 12px var(--ui);--tool-font:12.5px var(--ui);
--prose-font:400 16px/1.62 'Newsreader',Georgia,serif;--read:700px}
.dark{--bg:#151a1c;--main:#182022;--side:#131819;--card:#1d2528;--field:#232c2f;--hover:#232c2f;--sel:#20302a;
--ink:#e3eaec;--ink2:#c3ced1;--muted:#93a2a6;--faint:#66767a;--line:#273134;--line-soft:#1f282a;--line-strong:#313c3f;--accent:#8fc1a3;--accent-ink:#a6d1b7;--accent-soft:#1f2f28;--on-accent:#10201a;--ok:#8fc1a3;--bad:#e59486;--user-bg:#20303a;--shadow:none}
"""),
}

out = Path(__file__).parent
for k, d in DESIGNS.items():
    html = f"""<!doctype html><html lang="en"><head><meta charset="utf-8"><title>tazuna · {d['name']}</title>
<link rel="preconnect" href="https://fonts.googleapis.com"><link href="https://fonts.googleapis.com/css2?{d['fonts']}&display=swap" rel="stylesheet">
<style>{BASE}{d['css']}</style></head><body>{BODY.format(mark=d['mark'], **ICONS)}</body></html>"""
    (out / f"{k}.html").write_text(html)
cards = "".join(f'<a href="{k}.html"><b>{k.upper()} · {d["name"]}</b><span>{d["note"]}</span></a>' for k, d in DESIGNS.items())
(out / "index.html").write_text(f"""<!doctype html><html><head><meta charset="utf-8"><title>tazuna desktop · design directions</title>
<style>body{{font:14px/1.5 system-ui;max-width:760px;margin:48px auto;padding:0 20px;color:#222}}a{{display:block;padding:14px 16px;border:1px solid #ddd;border-radius:10px;margin:10px 0;color:inherit;text-decoration:none}}a:hover{{background:#f6f6f6}}span{{display:block;color:#666;margin-top:4px}}</style></head>
<body><h2>tazuna desktop: five design directions</h2><p>The same window in each (mockups, docs/DESKTOP.md). Each page has a light/dark switch at the bottom right.</p>{cards}</body></html>""")
print("wrote", ", ".join(f"{k}.html" for k in DESIGNS), "and index.html")
