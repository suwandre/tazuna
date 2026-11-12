The maintainers fixed it in https://github.com/withastro/astro/pull/18399 ("Skip server island manifest in CSS ensureModulesLoaded to fix late-discovery 500 in dev").

Their description:
## Changes

- In dev, a `server:defer` island that imports `astro:actions` or `astro:config/server` would 500 with `Could not find server component name` if its page wasn't the first island page loaded after the server started. `ensureModulesLoaded()` in the CSS plugin was walking into `virtual:astro:server-island-manifest` and re-fetching it on the server side before the module runner could, causing the runner to keep a stale island map that was missing the newly discovered island.
- `ensureModulesLoaded()` now skips `RESOLVED_SERVER_ISLAND_MANIFEST`. The manifest contains no CSS, so CSS collection is unaffected. The non-cascading refresh introduced in #18166 is preserved.

## Testing

- Added a dev-server integration test in `packages/astro/test/server-islands.test.ts` using a new `test/fixtures/server-islands/late-discovery` fixture. The test cold-starts the dev server, requests an island-free page first, then requests a second page whose island imports `astro:actions`, and asserts a 200 response (previously 500).

## Docs

- No docs update needed; this is a dev-only bug fix with no user-facing API changes.

Closes #18387

The fix (non-test files):
```diff
diff --git a/.changeset/wicked-melons-cheat.md b/.changeset/wicked-melons-cheat.md
new file mode 100644
index 000000000000..3c95bd692117
--- /dev/null
+++ b/.changeset/wicked-melons-cheat.md
@@ -0,0 +1,5 @@
+---
+'astro': patch
+---
+
+Fixes a dev server error (`Could not find server component name`) for a `server:defer` island that imports `astro:actions` or `astro:config/server`, when its page was not the first island page loaded after starting the server
diff --git a/packages/astro/src/core/server-islands/vite-plugin-server-islands.ts b/packages/astro/src/core/server-islands/vite-plugin-server-islands.ts
index a8d9e066a8ae..12d836efd93a 100644
--- a/packages/astro/src/core/server-islands/vite-plugin-server-islands.ts
+++ b/packages/astro/src/core/server-islands/vite-plugin-server-islands.ts
@@ -6,7 +6,7 @@ import { AstroError, AstroErrorData } from '../errors/index.js';
 import type { ServerIslandsState } from './shared-state.js';
 
 export const SERVER_ISLAND_MANIFEST = 'virtual:astro:server-island-manifest';
-const RESOLVED_SERVER_ISLAND_MANIFEST = '\0' + SERVER_ISLAND_MANIFEST;
+export const RESOLVED_SERVER_ISLAND_MANIFEST = '\0' + SERVER_ISLAND_MANIFEST;
 
 const serverIslandPlaceholderMap = "'$$server-islands-map$$'";
 const serverIslandPlaceholderNameMap = "'$$server-islands-name-map$$'";
diff --git a/packages/astro/src/vite-plugin-css/index.ts b/packages/astro/src/vite-plugin-css/index.ts
index 41a7946a0fca..375fef8aafed 100644
--- a/packages/astro/src/vite-plugin-css/index.ts
+++ b/packages/astro/src/vite-plugin-css/index.ts
@@ -10,6 +10,7 @@ import { getDevCSSModuleName } from './util.js';
 import { CSS_LANGS_RE } from '../core/viteUtils.js';
 import { PROPAGATED_ASSET_QUERY_PARAM } from '../content/consts.js';
 import { VIRTUAL_PAGE_RESOLVED_MODULE_ID } from '../vite-plugin-pages/const.js';
+import { RESOLVED_SERVER_ISLAND_MANIFEST } from '../core/server-islands/vite-plugin-server-islands.js';
 import {
 	ASTRO_CSS_EXTENSION_POST_PATTERN,
 	MODULE_DEV_CSS,
@@ -67,6 +68,12 @@ async function ensureModulesLoaded(
 		// page's module graph, fetching their CSS dependencies. This is the dev-mode
 		// counterpart of isBuildCssBoundary in plugin-css.ts. See #18060, #16116.
 		if (imp.id.startsWith(VIRTUAL_PAGE_RESOLVED_MODULE_ID)) continue;
+		// Skip the server island manifest. The server islands plugin refreshes it by clearing its
+		// transformResult so the module runner's next import re-transforms it and drops its stale
+		// copy. Fetching it here would repopulate transformResult first, so Vite would answer the
+		// runner with `cache: true` and it would keep an island map missing newly discovered
+		// islands. The manifest contains no CSS. See #18387.
+		if (imp.id === RESOLVED_SERVER_ISLAND_MANIFEST) continue;
 		// Skip virtual dev-css modules to prevent circular deadlocks with the Cloudflare adapter.
 		// The dev-css-all module statically imports all per-route dev-css:* modules, and those
 		// modules' load handlers may already be running (waiting on ensureModulesLoaded), causing
diff --git a/pnpm-lock.yaml b/pnpm-lock.yaml
index c43a29ba3e94..5b12c23bfa26 100644
--- a/pnpm-lock.yaml
+++ b/pnpm-lock.yaml
@@ -3925,6 +3925,12 @@ importers:
         specifier: ^5.57.2
         version: 5.57.2(@typescript-eslint/types@8.71.0)
 
+  packages/astro/test/fixtures/server-islands/late-discovery:
+    dependencies:
+      astro:
+        specifier: workspace:*
+        version: link:../../../..
+
   packages/astro/test/fixtures/server-islands/ssr:
     dependencies:
       '@astrojs/mdx':

```
A good answer includes this approach or an equally sound one, says why, and weighs at least one alternative.
