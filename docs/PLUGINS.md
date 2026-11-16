# tazuna desktop: panels and plugins (API 1)

You can change tazuna's window yourself, or ask tazuna to (`/create what you want`, or Cmd-K, Create a panel). Two kinds,
each a folder in `~/.config/tazuna/desktop/plugins/<id>/` (the id: lowercase letters, digits, `-`) with a
`manifest.json`: `{"title": "...", "description": "one line: what it does", "api": 1}`.

- A **panel** (`index.html`) is a card in the right column that shows the conversation's numbers. It is sandboxed: it
  cannot reach tazuna, the network or files, and needs no permission. Use a panel when showing information is enough.
- A **plugin** (`plugin.js`) runs inside the window: it can add parts anywhere, replace or hide tazuna's own parts,
  restyle the window, add Cmd-K actions and act (send a message, switch the permission mode). The user enables it
  (Settings, Panels and plugins); a change to its code outside creator mode needs enabling again. Use a plugin when the
  change touches the window itself or must act.

tazuna's own parts are registered in the same spots, the same way, so a plugin that works well can move into the app.

## Plugins

`plugin.js` is one ES module with no imports: its default export receives `ctx` and registers what it adds. Everything
it registers is taken back when it is turned off or reloaded; it may also return a function for its own clean-up.

```js
export default function (ctx) {
  const { h } = ctx;                      // React.createElement; ctx.React is React (hooks: ctx.React.useState ...)
  function Bar() {
    const d = ctx.useData();              // the conversation's data, the same object panels get (below)
    if (!d || !d.session) return null;
    return h("div", { style: { font: "12px var(--font-mono)", color: "var(--muted)", padding: "4px 2px" } },
      `${d.session.turns} turns · $${d.session.cost.toFixed(3)}`);
  }
  ctx.slots.register("composer.dock", { id: "cost-bar", order: 10, render: () => h(Bar) });
  ctx.palette.add({ id: "hello", label: "Say hello to the agent", run: () => ctx.actions.send("hello") });
}
```

### ctx

- `ctx.slots.register(spot, { id, order, render })`: adds a part; `render(props)` returns React elements (props per spot,
  below). Registering the id of one of tazuna's own parts **replaces** it. `order` sorts within the spot (tazuna's own
  use 10, 20, 30 ...; plugins default to 500).
- `ctx.slots.hide(id)`: removes one of tazuna's parts (or another plugin's).
- `ctx.slots.map()`: every spot in use now, with the ids in it and what provides each.
- `ctx.useData()`: a React hook returning the conversation's data (below), re-rendering on every change.
- `ctx.actions`: `send(text)` (a message to the agent, as if typed), `newChat()`, `setMode("auto"|"ask"|"edits"|"plan")`,
  `notify(title, body)` (a system notification), `openLink(url)` (http and https, in the browser),
  `review("uncommitted"|"branch")` (opens the review).
- `ctx.theme.set({ "--accent": "#b4473a" })`: design tokens (below) changed; `ctx.theme.css(text)`: a style sheet added.
- `ctx.palette.add({ id, label, hint, run })`: a Cmd-K action.
- `ctx.storage.get(key)` / `ctx.storage.set(key, value)`: small JSON settings kept for this plugin.

No network: the window's policy allows no requests to other hosts, for plugins too. A part that throws renders nothing
and its error shows in Settings.

### Spots

| Spot | What it is, and its props | tazuna's own parts there (ids) |
|---|---|---|
| `rail` | the right column's cards. props: { usage, data, messages } | rail.context, rail.plan, rail.thread, rail.balance, rail.today, rail.timeline (your messages; `window.dispatchEvent(new CustomEvent("tazuna-jump", {detail: i}))` scrolls to message i) |
| `header.actions` | the chips and buttons at the right of the header. props: { data, folder } | header.model, header.branch, header.review |
| `composer.dock` | above the message box (a bar, a dashboard). props: {} (the data: ctx.useData()) | (empty) |
| `composer.pills` | the controls under the message box (mode, model, effort, attach). props: { composer } | composer.mode, composer.model, composer.effort, composer.attach |
| `sidebar.top` | the sidebar's actions under the search (New chat, Open folder). props: {} | sidebar.new-chat, sidebar.open-folder |
| `sidebar.footer` | the sidebar's bottom row (Settings). props: { planLeft } | sidebar.settings |
| `message.user` | how one of your messages is drawn. props: { message, index } | message.user.default |
| `message.text` | how an answer's text is drawn (one entry: replace 'message.text.markdown' to restyle answers). props: { text } | message.text.markdown |
| `message.after` | under each answer (the change card, the cost line). props: { message, index } | message.change, message.meta |
| `tool.view` | how a tool step is drawn (one entry: replace 'tool.view.default'). props: { step } | tool.view.default |
| `overlay` | anything drawn over the window (fixed position). props: { data } | (empty) |

## Panels

A panel is a folder in `~/.config/tazuna/desktop/plugins/<id>/` (on Windows under the user's profile folder) holding:

- `manifest.json`: `{"title": "Cost by model", "slot": "rail", "api": 1}`. `title` shows over the panel; `slot` is where
  it goes (only `"rail"`, the right-hand column, for now); `api` the version below.
- `index.html`: the panel, one self-contained HTML file (its CSS in `<style>`, its code in `<script>`), at most 1 MB.

The id (the folder's name) is lowercase letters, digits and `-`. tazuna's own panels (context, plan, this thread)
follow the same rules, so a panel can move into the app as it is.

### What a panel may do

It runs sandboxed, apart from the window: no network (no fetch, no images or fonts from the web), no files, no access
to tazuna, keys or the page around it. A panel only receives data and draws it. Images and fonts must be inline
(`data:` URLs) or SVG drawn in code.

### The data it receives

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

### Looking like tazuna

The panel's page starts with tazuna's design tokens as CSS variables, updated with the theme: `--ink`, `--ink-2`,
`--muted`, `--faint`, `--line`, `--card`, `--field`, `--accent`, `--accent-soft`, `--ok`, `--bad`, `--warn`, and
`--font-ui`, `--font-mono`. The body already uses `--font-ui` at 12.5 px in `--ink` on a transparent background. Keep to
them (docs/DESKTOP.md and desktop/DESIGN.md: one accent, `ok` and `bad` only for results, no gradients or glow).

### Its height

A panel is 160 px tall until it says otherwise:

```js
parent.postMessage({ type: "tazuna-height", height: document.body.scrollHeight }, "*");
```

Between 40 and 600 px.

### A complete panel

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
