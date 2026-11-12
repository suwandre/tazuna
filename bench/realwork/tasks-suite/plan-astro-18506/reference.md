The maintainers fixed it in https://github.com/withastro/astro/pull/18506 ("Restore scroll position instantly on back/forward navigation when smooth scrolling is enabled").

Their description:
## Changes

- `<ClientRouter />` now passes `behavior: 'instant'` to all scroll-restore `scrollTo` calls in `moveToLocation()` and on page reload. Previously, omitting `behavior` caused these calls to inherit CSS `scroll-behavior: smooth`, producing a visible animated scroll to the saved position on back/forward navigation. The browser restores scroll instantly in the same scenario, so this brings `<ClientRouter />` in line with native behavior.
- Anchor (`#hash`) navigation is unaffected because it does not go through these `scrollTo` calls.

## Testing

- Added an e2e test in `packages/astro/e2e/view-transitions.test.ts` covering back/forward scroll restoration with `scroll-behavior: smooth` applied to `html`. The test asserts that `scrollY` reaches the saved position in at most one animation frame (i.e., no intermediate animated values).

## Docs

- No docs update needed; this is a bug fix that aligns `<ClientRouter />` with existing browser behavior, not a new option or API.

Closes #18505

The fix (non-test files):
```diff
diff --git a/.changeset/ninety-memes-start.md b/.changeset/ninety-memes-start.md
new file mode 100644
index 000000000000..c30927665ee7
--- /dev/null
+++ b/.changeset/ninety-memes-start.md
@@ -0,0 +1,5 @@
+---
+'astro': patch
+---
+
+Fixes `<ClientRouter />` animating scroll restoration on back/forward navigation when `scroll-behavior: smooth` is set. Saved scroll positions are now restored instantly, matching the browser's native behavior.
diff --git a/packages/astro/e2e/fixtures/view-transitions/src/pages/smooth-scroll.astro b/packages/astro/e2e/fixtures/view-transitions/src/pages/smooth-scroll.astro
new file mode 100644
index 000000000000..7b478280e894
--- /dev/null
+++ b/packages/astro/e2e/fixtures/view-transitions/src/pages/smooth-scroll.astro
@@ -0,0 +1,13 @@
+---
+import Layout from '../components/Layout.astro';
+---
+<Layout>
+	<article id="smooth-scroll" style="height: 5000px">
+		<a id="click-one" href="/one" style="position: fixed; top: 0">go to 1</a>
+	</article>
+</Layout>
+<style is:global>
+	html {
+		scroll-behavior: smooth;
+	}
+</style>
diff --git a/packages/astro/src/transitions/router.ts b/packages/astro/src/transitions/router.ts
index dfa77325908d..0a96182fd1e2 100644
--- a/packages/astro/src/transitions/router.ts
+++ b/packages/astro/src/transitions/router.ts
@@ -74,7 +74,7 @@ if (inBrowser) {
 		// Here we reloaded a page with history state
 		// (e.g. history navigation from non-transition page or browser reload)
 		currentHistoryIndex = history.state.index;
-		scrollTo({ left: history.state.scrollX, top: history.state.scrollY });
+		scrollTo({ left: history.state.scrollX, top: history.state.scrollY, behavior: 'instant' });
 	} else if (transitionEnabledOnThisPage()) {
 		// This page is loaded from the browser address bar or via a link from extern,
 		// it needs a state in the history
@@ -211,7 +211,7 @@ const moveToLocation = (
 	}
 
 	if (historyState) {
-		scrollTo(historyState.scrollX, historyState.scrollY);
+		scrollTo({ left: historyState.scrollX, top: historyState.scrollY, behavior: 'instant' });
 	} else {
 		if (to.hash) {
 			// because we are already on the target page ...

```
A good answer includes this approach or an equally sound one, says why, and weighs at least one alternative.
