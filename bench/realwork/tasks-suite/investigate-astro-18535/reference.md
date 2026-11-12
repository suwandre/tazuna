The maintainers fixed it in https://github.com/withastro/astro/pull/18535 ("fix(react): preserve component-rendered link tags in island output").

Their description:
## Changes

- Fixes #18418 by preserving component-rendered `<link>` tags (such as stylesheets and preconnect hints) in `@astrojs/react` island markup.
- React 19 auto-injects resource preloads (e.g., for `<img>` tags), which are intended to be stripped from island output. However, the regex previously stripped all `<link rel="...">` tags including `stylesheet` and `preconnect`, causing missing styles prior to hydration and React hydration mismatches (#418 / #423).
- Updated stripping logic to target only auto-injected preloads (`rel="preload|modulepreload"`) on React 19+.
- Skips stripping completely on React 18, which does not inject resource hints.
- Added changeset `.changeset/preserve-react-island-links.md`.

## Testing

- Updated `packages/integrations/react/test/react-19-preloads.test.ts` to assert that component-rendered `<link rel="stylesheet">` and `<link rel="preconnect">` are preserved in the rendered island markup while preloads remain stripped.
- Verified all `@astrojs/react` test suites pass cleanly.
- Biome formatting and lint checks pass.

## Docs

No documentation changes needed; this fixes unintended stripping of developer-rendered tags to align with expected component behavior.

The fix (non-test files):
```diff
diff --git a/.changeset/preserve-react-island-links.md b/.changeset/preserve-react-island-links.md
new file mode 100644
index 000000000000..ad055e7a6c11
--- /dev/null
+++ b/.changeset/preserve-react-island-links.md
@@ -0,0 +1,5 @@
+---
+'@astrojs/react': patch
+---
+
+Preserves component-rendered resource links, including stylesheets, preloads, and modulepreloads, in React island output. Automatic React resource hints are also preserved so that authored links are not removed during server rendering.
diff --git a/packages/astro/e2e/fixtures/react-resource-links/astro.config.mjs b/packages/astro/e2e/fixtures/react-resource-links/astro.config.mjs
new file mode 100644
index 000000000000..d790fbbdffe1
--- /dev/null
+++ b/packages/astro/e2e/fixtures/react-resource-links/astro.config.mjs
@@ -0,0 +1,7 @@
+import { defineConfig } from 'astro/config';
+import react from '@astrojs/react';
+
+export default defineConfig({
+  integrations: [react()],
+  compressHTML: 'jsx',
+});
diff --git a/packages/astro/e2e/fixtures/react-resource-links/package.json b/packages/astro/e2e/fixtures/react-resource-links/package.json
new file mode 100644
index 000000000000..7b4b04a63ec9
--- /dev/null
+++ b/packages/astro/e2e/fixtures/react-resource-links/package.json
@@ -0,0 +1,12 @@
+{
+  "name": "@test/react-resource-links",
+  "version": "0.0.0",
+  "private": true,
+  "type": "module",
+  "dependencies": {
+    "@astrojs/react": "workspace:*",
+    "astro": "workspace:*",
+    "react": "^19.3.0",
+    "react-dom": "^19.3.0"
+  }
+}
diff --git a/packages/astro/e2e/fixtures/react-resource-links/public/widget.css b/packages/astro/e2e/fixtures/react-resource-links/public/widget.css
new file mode 100644
index 000000000000..2f1cde58954c
--- /dev/null
+++ b/packages/astro/e2e/fixtures/react-resource-links/public/widget.css
@@ -0,0 +1 @@
+#widget { color: rgb(128, 0, 128); }
diff --git a/packages/astro/e2e/fixtures/react-resource-links/src/components/ImageComponent.jsx b/packages/astro/e2e/fixtures/react-resource-links/src/components/ImageComponent.jsx
new file mode 100644
index 000000000000..b6dc7f2a39c1
--- /dev/null
+++ b/packages/astro/e2e/fixtures/react-resource-links/src/components/ImageComponent.jsx
@@ -0,0 +1,14 @@
+export default function ImageComponent() {
+  return (
+    <div>
+      <link rel="stylesheet" href="/widget.css" />
+      <link rel="preconnect" href="https://example.com" />
+      <link rel="dns-prefetch" href="https://example.org" />
+      <link rel="preload" as="image" href="/explicit.png" />
+      <link rel="modulepreload" href="/explicit.js" />
+      <h1>React 19 Island</h1>
+      <button id="widget" onClick={(event) => { event.currentTarget.textContent = 'Hydrated'; }}>Widget</button>
+      <img src="/_astro/test-image.webp" alt="Test" />
+    </div>
+  );
+}
diff --git a/packages/astro/e2e/fixtures/react-resource-links/src/pages/index.astro b/packages/astro/e2e/fixtures/react-resource-links/src/pages/index.astro
new file mode 100644
index 000000000000..6df24b0504b1
--- /dev/null
+++ b/packages/astro/e2e/fixtures/react-resource-links/src/pages/index.astro
@@ -0,0 +1,11 @@
+---
+import ImageComponent from '../components/ImageComponent';
+---
+<html>
+<head>
+  <title>React 19 Test</title>
+</head>
+<body>
+  <ImageComponent client:load />
+</body>
+</html>
diff --git a/packages/integrations/react/src/server.ts b/packages/integrations/react/src/server.ts
index 46d1dee150ab..392f714e85d9 100644
--- a/packages/integrations/react/src/server.ts
+++ b/packages/integrations/react/src/server.ts
@@ -129,13 +129,11 @@ async function renderToStaticMarkup(
 	} else {
 		html = await renderToPipeableStreamAsync(vnode, renderOptions);
 	}
-	// Strip React 19 auto-injected resource hints (preloads, etc.) from island output.
-	// These should be in <head>, not inside the island.
-	// See: https://github.com/facebook/react/issues/27910
-	html = html.replace(
-		/<link\s[^>]*rel="(?:preload|modulepreload|stylesheet|preconnect|dns-prefetch)"[^>]*>/g,
-		'',
-	);
+	// Preserve React's resource links. Automatic hints and component-authored links
+	// can have identical markup, so filtering the HTML would also remove explicit
+	// stylesheets and preloads (and can break hydration with React 18).
+	// React handles its hoisted resources during hydration.
+	// See: https://github.com/withastro/astro/issues/18418
 	return { html, attrs };
 }
 
diff --git a/pnpm-lock.yaml b/pnpm-lock.yaml
index 071b58092199..47f2214a0e6a 100644
--- a/pnpm-lock.yaml
+++ b/pnpm-lock.yaml
@@ -1604,6 +1604,21 @@ importers:
         specifier: ^19.3.0
         version: 19.3.0(react@19.3.0)
 
+  packages/astro/e2e/fixtures/react-resource-links:
+    dependencies:
+      '@astrojs/react':
+        specifier: workspace:*
+        version: link:../../../../integrations/react
+      astro:
+        specifier: workspace:*
+        version: link:../../..
+      react:
+        specifier: ^19.3.0
+        version: 19.3.0
+      react-dom:
+        specifier: ^19.3.0
+        version: 19.3.0(react@19.3.0)
+
   packages/astro/e2e/fixtures/react19-preact-hook-error:
     dependencies:
       preact:
@@ -6118,10 +6133,10 @@ importers:
   packages/integrations/react/test/fixtures/react-19-preloads:
     dependencies:
       '@astrojs/react':
-        specifier: latest
+        specifier: workspace:*
         version: link:../../..
       astro:
-        specifier: latest
+        specifier: workspace:*
         version: link:../../../../../astro
       react:
         specifier: ^19.3.0

```
A good answer names this cause and where it is; another real cause, shown in the code, counts too.
