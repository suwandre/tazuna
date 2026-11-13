The maintainers fixed it in https://github.com/withastro/astro/pull/18336 ("Fix inlined Vue styles in dev serving untransformed (unscoped, raw v-bind) CSS").

Their description:
## Changes

- During `astro dev`, inline `<style data-vite-dev-id>` tags for Vue components now contain the correctly scoped, transformed CSS instead of the raw SFC `<style>` block. Previously, Astro cached CSS before `@vitejs/plugin-vue` could add `[data-v-xxx]` attribute selectors and convert `v-bind()` to CSS variables, causing a flash of unscoped styles and broken styling when JS is disabled.
- The CSS content cache hook is extracted into a new dev-only plugin (`astroDevCssContentCachePlugin`) and registered in `create-vite.ts` after all user and integration Vite config. This guarantees it runs after framework plugins (e.g. `vite:vue`) transform the CSS but before `vite:css-post` converts it to JS.

## Testing

- Adds a dev-server integration test in `packages/integrations/vue/test/basics.test.ts` that fetches the server-rendered HTML and asserts the inline style content matches the scoped, transformed output (attribute selectors and CSS variable form of `v-bind()`).

## Docs

- No docs update needed; this is an internal dev-server correctness fix with no API or configuration changes.

Closes #18333

The fix (non-test files):
```diff
diff --git a/.changeset/vue-dev-scoped-styles.md b/.changeset/vue-dev-scoped-styles.md
new file mode 100644
index 000000000000..c2e47df08ba6
--- /dev/null
+++ b/.changeset/vue-dev-scoped-styles.md
@@ -0,0 +1,5 @@
+---
+'astro': patch
+---
+
+Fixes inline styles in dev so that framework-transformed CSS is used, such as scoped styles and `v-bind()` in Vue single-file components
diff --git a/packages/astro/src/core/create-vite.ts b/packages/astro/src/core/create-vite.ts
index 1f6ee5551a93..03e7373e0bf9 100644
--- a/packages/astro/src/core/create-vite.ts
+++ b/packages/astro/src/core/create-vite.ts
@@ -30,7 +30,7 @@ import { vitePluginFetchable } from './fetch/vite-plugin.js';
 import astroVitePlugin from '../vite-plugin-astro/index.js';
 import { vitePluginAstroServer } from '../vite-plugin-astro-server/index.js';
 import configAliasVitePlugin from '../vite-plugin-config-alias/index.js';
-import { astroDevCssPlugin } from '../vite-plugin-css/index.js';
+import { astroDevCssContentCachePlugin, astroDevCssPlugin } from '../vite-plugin-css/index.js';
 import vitePluginFileURL from '../vite-plugin-fileurl/index.js';
 import astroHeadPlugin from '../vite-plugin-head/index.js';
 import astroHmrReloadPlugin from '../vite-plugin-hmr-reload/index.js';
@@ -155,8 +155,8 @@ export async function createVite(
 		config: settings.config,
 	});
 	const serverIslandsState = new ServerIslandsState();
-	// Shared cache of CSS content by module ID. Populated by the dev-css plugin's
-	// transform hook and consumed by the content asset propagation plugin to avoid
+	// Shared cache of CSS content by module ID. Populated by the dev-css content cache
+	// plugin's transform hook and consumed by the content asset propagation plugin to avoid
 	// re-processing CSS modules with `?inline` (which produces different
 	// scoped-name hashes with Lightning CSS).
 	const cssContentCache = new Map<string, string>();
@@ -368,6 +368,12 @@ export async function createVite(
 		result = vite.mergeConfig(result, settings.config.vite || {});
 	}
 	result = vite.mergeConfig(result, commandConfig);
+	if (command === 'dev') {
+		// Appended last so it sees CSS after integration plugins have transformed it.
+		result = vite.mergeConfig(result, {
+			plugins: [astroDevCssContentCachePlugin({ cssContentCache })],
+		});
+	}
 
 	return result;
 }
diff --git a/packages/astro/src/vite-plugin-css/index.ts b/packages/astro/src/vite-plugin-css/index.ts
index 41a7946a0fca..c7667c868011 100644
--- a/packages/astro/src/vite-plugin-css/index.ts
+++ b/packages/astro/src/vite-plugin-css/index.ts
@@ -273,27 +273,6 @@ export function astroDevCssPlugin({
 					}
 				},
 			},
-
-			transform: {
-				filter: {
-					id: {
-						include: [CSS_LANGS_RE],
-						exclude: [rawRE, inlineRE],
-					},
-				},
-				handler(code, id) {
-					if (command === 'build') {
-						return;
-					}
-
-					// Cache CSS content as we see it
-					const env = getCurrentEnvironment(this.environment as DevEnvironment);
-					const mod = env?.moduleGraph.getModuleById(id);
-					if (mod) {
-						cssContentCache.set(id, code);
-					}
-				},
-			},
 		},
 		{
 			name: MODULE_DEV_CSS_ALL,
@@ -332,3 +311,41 @@ export function astroDevCssPlugin({
 		},
 	];
 }
+
+/**
+ * Caches the final CSS of each module in `cssContentCache` during dev, so it can be inlined
+ * into server-rendered HTML.
+ *
+ * Must be registered after all integration and user plugins: its normal-order `transform` then
+ * runs after framework transforms (e.g. `@vitejs/plugin-vue` scoping SFC styles) and before
+ * `vite:css-post` turns the CSS into JS. See #18333.
+ */
+export function astroDevCssContentCachePlugin({
+	cssContentCache,
+}: Pick<AstroVitePluginOptions, 'cssContentCache'>): Plugin {
+	return {
+		name: 'astro:dev-css-content-cache',
+		applyToEnvironment(env) {
+			return (
+				env.name === ASTRO_VITE_ENVIRONMENT_NAMES.astro ||
+				env.name === ASTRO_VITE_ENVIRONMENT_NAMES.ssr ||
+				env.name === ASTRO_VITE_ENVIRONMENT_NAMES.client ||
+				env.name === ASTRO_VITE_ENVIRONMENT_NAMES.prerender
+			);
+		},
+		transform: {
+			filter: {
+				id: {
+					include: [CSS_LANGS_RE],
+					exclude: [rawRE, inlineRE],
+				},
+			},
+			handler(code, id) {
+				const mod = (this.environment as DevEnvironment).moduleGraph.getModuleById(id);
+				if (mod) {
+					cssContentCache.set(id, code);
+				}
+			},
+		},
+	};
+}

```
A good answer names this cause and where it is; another real cause, shown in the code, counts too.
