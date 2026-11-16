# tazuna desktop panels (widget API 1)

A panel is a folder in `~/.config/tazuna/desktop/widgets/<id>/` (on Windows under the user's profile folder) holding:

- `manifest.json`: `{"title": "Cost by model", "slot": "rail", "api": 1}`. `title` shows over the panel; `slot` is where
  it goes (only `"rail"`, the right-hand column, for now); `api` the version below.
- `index.html`: the panel, one self-contained HTML file (its CSS in `<style>`, its code in `<script>`), at most 1 MB.

The id (the folder's name) is lowercase letters, digits and `-`. tazuna's own panels (context, plan, this thread)
follow the same rules, so a panel can move into the app as it is.

## What a panel may do

It runs sandboxed, apart from the window: no network (no fetch, no images or fonts from the web), no files, no access
to tazuna, keys or the page around it. A panel only receives data and draws it. Images and fonts must be inline
(`data:` URLs) or SVG drawn in code.

## The data it receives

tazuna posts a message to the panel whenever the conversation on screen changes, and once when the panel loads:

```js
window.addEventListener("message", (e) => {
  if (e.data && e.data.type === "tazuna") draw(e.data.data);
});
```

`e.data.data` (API 1):

```js
{
  api: 1,
  theme: "light" | "dark",
  session: {
    model: "claude-opus-5-5", effort: "high", mode: "auto",      // permission mode: auto, ask, edits, plan
    activity: "Thinking",                                         // Ready, Starting, Waiting, Thinking, Writing, Tool, Verifying, Done
    cwd: "/path/to/project", profile: "repo",
    cost: 0.042,                                                  // dollars, this conversation
    turns: 7,                                                     // model calls, this conversation
    context: 48210, context_window: 200000,                       // tokens in context now, and the model's window
    usage: { input: 1200, cache_write: 9000, cache_read: 81000, output: 3400 },   // tokens, this conversation
    limits: [5, 31] | null,                                       // Claude plan used, percent: 5-hour, week
    limits_reset: 1794600000 | null,                              // when the 5-hour window resets (Unix seconds)
    balance: 0.32 | null                                          // SubRouter balance in dollars
  },
  turns: [ { cost: 0.004, secs: 12, tokens: 9100, cached: 0.88, model: "claude-opus-5-5" } ],   // each finished turn, oldest first
  tools: { "Read": 12, "Bash": 4 },                               // calls per tool, this conversation
  changes: [ { turn: 0, files: [ { path: "src/a.ts", added: 12, removed: 3 } ] } ]           // files each turn changed
}
```

Fields can be `null` where tazuna does not know them (no Claude plan reading yet, no SubRouter key). New fields may be
added within API 1; none are removed or renamed.

## Looking like tazuna

The panel's page starts with tazuna's design tokens as CSS variables, updated with the theme: `--ink`, `--ink-2`,
`--muted`, `--faint`, `--line`, `--card`, `--field`, `--accent`, `--accent-soft`, `--ok`, `--bad`, `--warn`, and
`--font-ui`, `--font-mono`. The body already uses `--font-ui` at 12.5 px in `--ink` on a transparent background. Keep to
them (docs/DESKTOP.md and desktop/DESIGN.md: one accent, `ok` and `bad` only for results, no gradients or glow).

## Its height

A panel is 160 px tall until it says otherwise:

```js
parent.postMessage({ type: "tazuna-height", height: document.body.scrollHeight }, "*");
```

Between 40 and 600 px.

## A complete example

```html
<style>
  .n { font: 600 22px var(--font-ui); letter-spacing: -0.02em; }
  .s { color: var(--muted); margin-top: 4px; }
</style>
<div class="n" id="n">–</div>
<div class="s">output tokens, this conversation</div>
<script>
  window.addEventListener("message", (e) => {
    if (!e.data || e.data.type !== "tazuna") return;
    document.getElementById("n").textContent = e.data.data.session.usage.output.toLocaleString();
    parent.postMessage({ type: "tazuna-height", height: document.body.scrollHeight }, "*");
  });
</script>
```
