The maintainers fixed it in https://github.com/withastro/astro/pull/18608 ("fix(astro): skip data-store full-reload when no clients are connected").

Their description:
Fixes #18568

`attachDataStoreInvalidation` always called `invalidateDataStore` with `notifyClient: true`. In `astro dev`, the initial content-layer sync writes the data store before the server is listening. Vite buffers that `full-reload` and delivers it right after the first client `connected`, so the first page loads twice on a fresh start.

`dev()` now passes a `shouldNotifyClient` callback that returns `false` while the initial sync runs. Server-side invalidation still happens. `sync()` awaits `waitUntilSaveComplete()`, so the startup write finishes before the flag flips. Writes after startup still reload, including ones Vite buffers for a temporarily disconnected browser. This mirrors `buildStart`, which already skips the client notify (#17286).

## Test plan
- [x] Unit: a write during initial sync invalidates the server but sends no client `full-reload`. A later write does reload.
- [x] Existing invalidation, watcher-echo and prerender tests still pass

The fix (non-test files):
```diff
diff --git a/.changeset/content-startup-sync-reloads.md b/.changeset/content-startup-sync-reloads.md
new file mode 100644
index 000000000000..c244454bf950
--- /dev/null
+++ b/.changeset/content-startup-sync-reloads.md
@@ -0,0 +1,5 @@
+---
+'astro': patch
+---
+
+Fixes an extra page reload on the first page load after starting `astro dev` without an existing `.astro` directory
diff --git a/packages/astro/src/content/vite-plugin-content-virtual-mod.ts b/packages/astro/src/content/vite-plugin-content-virtual-mod.ts
index 731750b1a57a..c3c062cbd724 100644
--- a/packages/astro/src/content/vite-plugin-content-virtual-mod.ts
+++ b/packages/astro/src/content/vite-plugin-content-virtual-mod.ts
@@ -136,18 +136,21 @@ function getDevDataStoreFile(settings: AstroSettings): URL {
  * write on some platforms (notably Windows, see #17335), leaving dev serving
  * stale content until a restart. Subscribing to the store's own write
  * notifications makes invalidation of this process's writes deterministic.
+ * The caller can suppress client reloads during initial sync without skipping
+ * server-side invalidation.
  */
 export function attachDataStoreInvalidation(
 	store: MutableDataStore,
 	server: ViteDevServer,
 	settings: AstroSettings,
+	{ shouldNotifyClient = () => true }: { shouldNotifyClient?: () => boolean } = {},
 ) {
 	const dataStorePath = fileURLToPath(getDevDataStoreFile(settings));
 	const assetImportsPath = fileURLToPath(new URL(ASSET_IMPORTS_FILE, settings.dotAstroDir));
 	store.onFileWritten((path) => {
 		if (path === dataStorePath) {
 			markDirectInvalidation(dataStorePath);
-			invalidateDataStore(server);
+			invalidateDataStore(server, { notifyClient: shouldNotifyClient() });
 			invalidateAssetImports(server, assetImportsPath);
 		} else if (path === assetImportsPath) {
 			markDirectInvalidation(assetImportsPath);
diff --git a/packages/astro/src/core/dev/dev.ts b/packages/astro/src/core/dev/dev.ts
index 9211ee89b22a..ea1cc29aefff 100644
--- a/packages/astro/src/core/dev/dev.ts
+++ b/packages/astro/src/core/dev/dev.ts
@@ -103,13 +103,18 @@ export default async function dev(inlineConfig: AstroInlineConfig): Promise<DevS
 		logger.error('content', err.message);
 	}
 
+	let initialContentSync = true;
 	if (!store) {
 		logger.error('content', 'Failed to create data store');
 	} else {
 		// Invalidate the content virtual modules directly when the store is
 		// written, rather than relying on the file watcher to observe the write.
 		// On Windows the watcher can miss it, leaving dev serving stale content.
-		attachDataStoreInvalidation(store, restart.container.viteServer, restart.container.settings);
+		attachDataStoreInvalidation(store, restart.container.viteServer, restart.container.settings, {
+			// Vite buffers startup reloads and delivers them to the first browser,
+			// even though it has not loaded any content yet (#18568).
+			shouldNotifyClient: () => !initialContentSync,
+		});
 	}
 	await attachContentServerListeners(restart.container);
 
@@ -130,6 +135,10 @@ export default async function dev(inlineConfig: AstroInlineConfig): Promise<DevS
 		logger.warn('content', 'Content config not loaded');
 	}
 
+	// Runtime writes must queue reloads even before the first HMR connection,
+	// or while clients are temporarily disconnected. Only initial sync is silent.
+	initialContentSync = false;
+
 	// Start listening to the port
 	const devServerAddressInfo = await startContainer(restart.container);
 

```
A good answer names this cause and where it is; another real cause, shown in the code, counts too.
