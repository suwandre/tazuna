# Routes: checking, picking and switching providers in the own loop (design, 2026-10-23)

## Why

Three days of gateway benchmarks (CHANGELOG 2026-10-21 to -23) found the route moving results more than the model, the
effort or a review step: Luna judged 2-6 on one SubRouter route and 8-8.5 on another; on one key a run landed on a pool at
8-16 s a call or on another at 56-148 s and stayed there; SubRouter refused requests carrying `prompt_cache_key` for an
hour (503 "no available channel"), kept a switched-off provider in a key's routing, and moved a DeepSeek run to a
provider that demanded `reasoning_content`. tazuna handles one of these today (UnoRouter group switching on hard errors and
under 70% cached); everything else is done by hand with `bench/uno_route.py` and the bench scripts.

## What exists (0.0.68-0.0.78)

- `Groups` (engine.rs): one list of UnoRouter groups per engine, from `TAZUNA_GROUPS`/`TAZUNA_GROUPS_FILE`, applied as an
  `X-Group` header. Switches at once on `get_channel_failed`, "no available provider" and two 400s; moves on when the last
  10 calls cached under 70% (never off the last group). Logged as `group_switch` (free text). No group left: calls go out
  without the header.
- Per run and fixed: `--url`, `--key-env` (read once, then removed from the environment), the `Api` (from the model name
  and the provider's model list), the price (`TAZUNA_PRICE` or `/models`). Provider quirks already follow the URL
  (Mortera, UnoRouter, Shannon in `chat_in`).
- Per run and sticky: `cache_key` off after a refusal (0.0.78), `send_reasoning` on after a demand (0.0.77).
- Not measured: the time of each call. Retries: 5xx/429 back off on the same route for up to ~15 minutes (429: 70 more).
- Bench only: the route check (cache over a growing conversation, a long reasoning call, language and identity), price
  checks, refilling the list, stopping on price drift or low cache.

## Design

**A route** is `url, key env, optional group, optional price`, for one model. Routes are listed in
`~/.config/tazuna/routes` (one per line, like `presets` and `trusted-providers`):

    gpt-6-luna      https://subrouter.ai/v1  SUBROUTER_LUNA4_KEY   price=0.02,0.002,0.1   # LingAI
    gpt-6-luna      https://subrouter.ai/v1  SUBROUTER_LUNA3_KEY                          # Pro pool
    deepseek-v4.1-flash-1017  https://api.unorouter.com/v1  UNOROUTER_API_KEY  group=a7-zlkpro-api-5857-...

`--provider`/`--key-env` keep working and mean a list of one. UnoRouter groups become routes of their own (the group list
is folded in, `TAZUNA_GROUPS*` kept as input).

**At the start** nothing extra: the run's first call is the check (no probe call, no extra tokens). A refusal or failure
there moves to the next route like any later call. The bench's full check stays in `uno_route.py` for building the list.

**During the run**, the current route is left (for the next in the list; none left: the run ends with a clear message, not
calls without a route) when:
1. a refusal that will not pass: the `Groups` errors, "no available channel"/`model_not_found` after the cache-key
   fallback, 401/403;
2. 5xx/429 three times in a row (not 15 minutes of back-off: another route is the better wait);
3. the last 10 calls cached under 70% (today's rule, now for any route);
4. **slow**: the median of the last 10 calls over 3x the run's first 10, and over 40 s (one run of a pool at 56-148 s a
   call against 8-16 s on the other). Needs the call time: an `Instant` around `(self.call)`, added to `Groups.recent`.
5. empty replies three times in a row (today retried on the same route).

**Cost**: the rules read what the run's own calls return (errors, cached tokens, time); a switch costs one cold
cache (one call's history uncached, ~50-100K tokens, 0.1-1 cent on these routes), and only when the route is already
failing. Staying cost more: a slow pool 3 hours, a 0%-cache route 4-10x the input, a refusal the run.

**On a switch** the history goes with the conversation, minus what is bound to the old upstream: Claude thinking blocks
(`drop_thinking`, exists), Responses `reasoning` items (encrypted per organisation: a new `drop_reasoning`), and the
per-route flags (`cache_key`, `send_reasoning`, effort fallback) reset. The cache starts cold on the new route; that cost is
why switching for a lower price is not a rule.

**Keys** of every route are read at the start and removed from the environment, as today for the one key. `check_data`
applies to every route's host.

**Logged**: `route_switch` with fields (`from`, `to`, `why`, `calls_on_from`, `median_s`, `cached`), and the route on each
call in the session log, so benchmarks and real use say which route served what (today only SubRouter's export does).

## Measurement

1. Unit tests per rule (a fake `call` as in `cache_key_dropped_when_refused`).
2. A bench run with a bad route first in the list (the Team-pool key, or a key without the model) and a good one second:
   the run must switch within the rules and finish like a run on the good route alone.
3. The same tasks with the list and with its best route alone, a few runs each: the list must not cost quality or more than
   ~10% cost (the cold cache after a switch).

## Not in this step

Picking routes by price or by our judge scores (the model picker per role, ROADMAP Next 5) and refilling the list in the
background: they need this step's switching and logging first. Best of two (CHANGELOG 2026-10-22/23) waits for it too: its
live test was spoiled by a slow pool.
