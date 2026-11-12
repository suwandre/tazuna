The maintainers fixed it in https://github.com/withastro/astro/pull/18630 ("Emit an import map with integrity hashes so strictDynamic allows Astro's bundled scripts and hydrated islands to load").

Their description:
## Changes

- When `security.csp.scriptDirective.strictDynamic` is enabled, Astro now emits a `<script type="importmap">` in `<head>` (before any module scripts) that maps every client `.js`/`.mjs` asset URL to its SHA digest. Browsers propagate the integrity metadata through dynamic imports, so `<script src>` entry points, the chunks they import, and `import()` calls from `<astro-island>` all load without being blocked by `'strict-dynamic'`.
- The hash of the import map itself, plus the hash of every hashed file, is added to `script-src`. A new `trackScriptIntegrity()` helper in `core/csp/common.ts` reads files from the client output on disk (avoiding the `for...in`-over-`Set` bug in the old `trackScriptHashes`/`trackStyleHashes` loops that prevented any chunk hashes from reaching the policy). The dead `for...in` loops are removed. Pages without `strictDynamic` are unaffected.

## Testing

- New unit tests in `packages/astro/test/units/csp/common.test.ts` cover `trackScriptIntegrity()`: hash generation, import map structure, deterministic ordering, and query-string variants for adapters with `assetQueryParams`.
- Integration tests added in `packages/astro/test/csp.test.ts` verify that a built page with `strictDynamic: true` contains a `<script type="importmap">` with the expected integrity entries and that all file hashes appear in `script-src`.

## Docs

- The `strictDynamic` JSDoc in `types/public/config.ts` is extended to explain the import map behaviour and the minimum browser versions required (Chrome 127, Firefox 138, Safari 18). No separate docs-site update is needed beyond that inline note.

Closes #18607

The fix (non-test files):
```diff
diff --git a/.changeset/csp-strict-dynamic-import-map.md b/.changeset/csp-strict-dynamic-import-map.md
new file mode 100644
index 000000000000..ecd70a2e6a0d
--- /dev/null
+++ b/.changeset/csp-strict-dynamic-import-map.md
@@ -0,0 +1,6 @@
+---
+'astro': patch
+'@astrojs/runtime': patch
+---
+
+Fixes bundled scripts and hydrated islands being blocked when `security.csp.scriptDirective.strictDynamic` is enabled. Astro now adds the hash of every bundled script to `script-src` and renders an import map with their `integrity` in the `<head>`.
diff --git a/packages/astro/src/core/app/types.ts b/packages/astro/src/core/app/types.ts
index bd39face3500..8b7c16cecd6d 100644
--- a/packages/astro/src/core/app/types.ts
+++ b/packages/astro/src/core/app/types.ts
@@ -205,6 +205,8 @@ export type SSRManifestCSP = {
 	};
 	/** Static speculation rules JSON to inject in the head when CSP + clientPrerender are both enabled. */
 	speculationRulesContent?: string;
+	/** Import map JSON whose `integrity` map covers every client script, injected in the head when `strictDynamic` is enabled. */
+	importMapContent?: string;
 };
 
 export interface SSRManifestSession extends BaseSessionConfig {
diff --git a/packages/astro/src/core/build/plugins/plugin-manifest.ts b/packages/astro/src/core/build/plugins/plugin-manifest.ts
index 0ef662a65553..2752b6846527 100644
--- a/packages/astro/src/core/build/plugins/plugin-manifest.ts
+++ b/packages/astro/src/core/build/plugins/plugin-manifest.ts
@@ -26,6 +26,7 @@ import {
 	getStyleResources,
 	shouldTrackCspHashes,
 	trackScriptHashes,
+	trackScriptIntegrity,
 	trackStyleHashes,
 } from '../../csp/common.js';
 import { partitionByKind } from '../../csp/runtime.js';
@@ -147,7 +148,13 @@ async function createManifest(
 
 	const staticFiles = internals.staticFiles;
 	const encodedKey = await encodeKey(await buildOpts.key);
-	const manifest = await buildManifest(buildOpts, internals, Array.from(staticFiles), encodedKey);
+	const manifest = await buildManifest(
+		buildOpts,
+		internals,
+		Array.from(staticFiles),
+		encodedKey,
+		clientStatics,
+	);
 	return manifest;
 }
 
@@ -201,6 +208,7 @@ async function buildManifest(
 	internals: BuildInternals,
 	staticFiles: string[],
 	encodedKey: string,
+	clientFiles: Set<string>,
 ): Promise<SerializedSSRManifest> {
 	const { settings } = opts;
 
@@ -349,7 +357,6 @@ async function buildManifest(
 				internals,
 				settings,
 				algorithm,
-				clientRoot: opts.outputDirectories.client,
 			})),
 		];
 		const styleHashes = [
@@ -359,10 +366,24 @@ async function buildManifest(
 				internals,
 				settings,
 				algorithm,
-				clientRoot: opts.outputDirectories.client,
 			})),
 		];
 
+		let importMapContent: string | undefined;
+		if (getStrictDynamic(cspConfig)) {
+			const scriptIntegrity = await trackScriptIntegrity({
+				settings,
+				algorithm,
+				clientRoot: opts.outputDirectories.client,
+				clientFiles,
+				assetQueryString,
+			});
+			if (scriptIntegrity) {
+				importMapContent = scriptIntegrity.importMapContent;
+				scriptHashes.push(...scriptIntegrity.hashes);
+			}
+		}
+
 		// When both CSP and clientPrerender are enabled, generate a static speculation rules
 		// script whose hash can be included in the CSP policy. Dynamic per-URL injection would
 		// produce unpredictable hashes that cannot be whitelisted at build time.
@@ -402,6 +423,7 @@ async function buildManifest(
 			scriptDirective,
 			styleDirective,
 			speculationRulesContent,
+			importMapContent,
 		};
 	}
 
diff --git a/packages/astro/src/core/csp/common.ts b/packages/astro/src/core/csp/common.ts
index 983b9e2f1145..5e8ae5969bb9 100644
--- a/packages/astro/src/core/csp/common.ts
+++ b/packages/astro/src/core/csp/common.ts
@@ -6,6 +6,7 @@ import { ISLAND_STYLES } from '@astrojs/runtime/_internal/server/astro-island-st
 import type { AstroSettings } from '../../types/astro.js';
 import type { AstroConfig, CspAlgorithm } from '../../types/public/index.js';
 import type { BuildInternals } from '../build/internal.js';
+import { createAssetLink } from '../render/ssr-element.js';
 import { generateCspDigest } from '@astrojs/runtime/_internal/core/encryption.js';
 import type { CspDirective, CspHash, CspHashEntry, CspResourceEntry } from './config.js';
 
@@ -96,14 +97,12 @@ interface TrackCspHashesOptions {
 	internals: BuildInternals;
 	settings: AstroSettings;
 	algorithm: CspAlgorithm;
-	clientRoot: URL;
 }
 
 export async function trackStyleHashes({
 	internals,
 	settings,
 	algorithm,
-	clientRoot,
 }: TrackCspHashesOptions): Promise<CspHash[]> {
 	const clientStyleHashes: CspHash[] = [];
 	for (const [_, page] of internals.pagesByViteID.entries()) {
@@ -114,13 +113,6 @@ export async function trackStyleHashes({
 		}
 	}
 
-	for (const clientAsset in internals.clientChunksAndAssets) {
-		const contents = readFileSync(fileURLToPath(new URL(clientAsset, clientRoot)), 'utf-8');
-		if (clientAsset.endsWith('.css') || clientAsset.endsWith('.css')) {
-			clientStyleHashes.push(await generateCspDigest(contents, algorithm));
-		}
-	}
-
 	if (settings.renderers.length > 0) {
 		clientStyleHashes.push(await generateCspDigest(ISLAND_STYLES, algorithm));
 	}
@@ -132,7 +124,6 @@ export async function trackScriptHashes({
 	internals,
 	settings,
 	algorithm,
-	clientRoot,
 }: TrackCspHashesOptions): Promise<CspHash[]> {
 	const clientScriptHashes: CspHash[] = [];
 
@@ -144,13 +135,6 @@ export async function trackScriptHashes({
 		clientScriptHashes.push(await generateCspDigest(directiveContent, algorithm));
 	}
 
-	for (const clientAsset in internals.clientChunksAndAssets) {
-		const contents = readFileSync(fileURLToPath(new URL(clientAsset, clientRoot)), 'utf-8');
-		if (clientAsset.endsWith('.js') || clientAsset.endsWith('.mjs')) {
-			clientScriptHashes.push(await generateCspDigest(contents, algorithm));
-		}
-	}
-
 	for (const script of settings.scripts) {
 		const { content, stage } = script;
 		if (stage === 'head-inline' || stage === 'before-hydration
[diff truncated]
```
A good answer names this cause and where it is; another real cause, shown in the code, counts too.
