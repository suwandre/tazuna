The PR as its author described it (https://github.com/n8n-io/n8n/pull/43360):
# Description
Backport of #43231 to `release-candidate/2.47.x`.

## Checklist for the author (@looorent) to go through.

- [ ] Review the backport changes
- [ ] Fix possible conflicts
- [ ] Merge to target branch

After this PR has been merged, it will be picked up in the next patch release for release track.

# Original description

## Summary

An expression can replace a JavaScript built-in that n8n also uses to transfer results. Consider these two expressions in the same execution:

```javascript
// First expression: replace Object.keys and return 'x'.
{{ (Object.keys = () => { throw new Error('replaced'); }, 'x') }}

// Next expression: return an object.
{{ ({ b: 2 }) }}
```

**Before:** the first expression returns `'x'`. The second fails when the runtime calls the replaced `Object.keys` to copy the object back to n8n.

**After:** the second expression returns `{ b: 2 }`.

Affected engines: `vm` and `quickjs`

## Why

The JavaScript environment is reused across expressions in one execution. A later expression can fail even when it never calls the replaced function itself.

## Known limit 

Luxon still reads built-ins such as `Number.isFinite` in its own code. Saving references in our runtime does not change those calls.

## Related Linear tickets, Github issues, and Community forum posts

https://linear.app/n8n/issue/CET-589

## Review / Merge checklist

- [x] I have seen this code, I have run this code, and I take responsibility for this code.
- [x] PR title and summary are descriptive. ([conventions](../blob/master/.github/pull_request_title_conventions.md))
- [x] Tests included.
- [x] PR Labeled with `Backport to Beta`, `Backport to Stable`, or `Backport to v1` (if the PR is an urgent fix that needs to be backported)

<!-- This is an auto-generated description by cubic. -->
<a href="https://cubic.dev/pr/n8n-io/n8n/pull/43231?utm_source=github" target="_blank" rel="noopener noreferrer" data-no-image-dialog="true"><picture><source media="(prefers-color-scheme: dark)" srcset="https://www.cubic.dev/buttons/review-in-cubic-dark.svg"><source media="(prefers-color-scheme: light)" srcset="https://www.cubic.dev/buttons/review-in-cubic-light.svg"><img alt="View guided diff" src="https://www.cubic.dev/buttons/review-in-cubic-light.svg"></picture></a> <a href="https://www.cubic.dev/action/auto-fix/pr/n8n-io/n8n/43231?returnTo=https%3A%2F%2Fgithub.com%2Fn8n-io%2Fn8n%2Fpull%2F43231&source=description" target="_blank" rel="no

Its changes (non-test files):
```diff
diff --git a/packages/@n8n/expression-runtime/src/bridge/quickjs-bridge.ts b/packages/@n8n/expression-runtime/src/bridge/quickjs-bridge.ts
index bbf48bb472f4..cebba9df1947 100644
--- a/packages/@n8n/expression-runtime/src/bridge/quickjs-bridge.ts
+++ b/packages/@n8n/expression-runtime/src/bridge/quickjs-bridge.ts
@@ -708,6 +708,10 @@ export class QuickJsBridge implements RuntimeBridge {
 
 		const shimCode = `
 (function() {
+	// An expression can replace a built-in, so keep the ones from context setup.
+	var isArray = Array.isArray;
+	var objectKeys = Object.keys;
+
 	function checkError(result) {
 		if (result && typeof result === 'object' && result.__intlError) {
 			throw new Error(result.message);
@@ -725,7 +729,7 @@ export class QuickJsBridge implements RuntimeBridge {
 		return copy;
 	}
 	function snapshotLocales(locales) {
-		return Array.isArray(locales) ? locales.slice() : locales;
+		return isArray(locales) ? locales.slice() : locales;
 	}
 
 	function callIntl(inst, ctorName, op, args, transform) {
@@ -787,7 +791,7 @@ export class QuickJsBridge implements RuntimeBridge {
 	// prefers getWeekInfo() — expose both, like current V8.
 	function Locale(tag, options) {
 		var dumped = checkError(__intl('Locale', 'dump', tag, snapshotOptions(options)));
-		var keys = Object.keys(dumped);
+		var keys = objectKeys(dumped);
 		for (var i = 0; i < keys.length; i++) {
 			var k = keys[i];
 			if (k === 'weekInfo') this._weekInfo = dumped[k];
@@ -930,11 +934,14 @@ export class QuickJsBridge implements RuntimeBridge {
 
 		const wrapperCode = `
 (function() {
-	// Host data uses the parser from context setup.
+	// An expression can replace a built-in, so keep the ones from context setup.
 	var parseJSON = JSON.parse;
+	var defineProperty = Object.defineProperty;
+	var objectKeys = Object.keys;
+	var isArray = Array.isArray;
 	// Define own data keys so a JSON __proto__ key stays part of the data.
 	function defineOwnValue(target, key, value) {
-		Object.defineProperty(target, key, { value: value, writable: true, enumerable: true, configurable: true });
+		defineProperty(target, key, { value: value, writable: true, enumerable: true, configurable: true });
 	}
 	var original = __prepareForTransfer;
 	globalThis.__prepareForTransfer = function(value) {
@@ -964,7 +971,7 @@ export class QuickJsBridge implements RuntimeBridge {
 		}
 		if (v instanceof Error) {
 			var errExtra = {};
-			var errKeys = Object.keys(v);
+			var errKeys = objectKeys(v);
 			for (var ei = 0; ei < errKeys.length; ei++) {
 				if (errKeys[ei] !== 'name' && errKeys[ei] !== 'message' && errKeys[ei] !== 'stack') {
 					defineOwnValue(errExtra, errKeys[ei], wrapSpecialValues(v[errKeys[ei]], true));
@@ -986,7 +993,7 @@ export class QuickJsBridge implements RuntimeBridge {
 			});
 			return { __isSet: true, __values: values };
 		}
-		if (Array.isArray(v)) return v.map(function(item) { return wrapChild(item, inCollection); });
+		if (isArray(v)) return v.map(function(item) { return wrapChild(item, inCollection); });
 		// Outside a collection, these markers come from __prepareForTransfer above,
 		// so pass them to the host as they are.
 		if (!inCollection) {
@@ -1005,7 +1012,7 @@ export class QuickJsBridge implements RuntimeBridge {
 					opaqueWrapper['${TRANSFER_OPAQUE_KEY}'] = true;
 					return opaqueWrapper;
 				}
-				if (payload !== null && typeof payload === 'object' && !Array.isArray(payload)) {
+				if (payload !== null && typeof payload === 'object' && !isArray(payload)) {
 					var walkedWrapper = { __value: wrapOwnKeys(payload, inCollection) };
 					walkedWrapper['${TRANSFER_ESCAPED_KEY}'] = true;
 					return walkedWrapper;
@@ -1020,7 +1027,7 @@ export class QuickJsBridge implements RuntimeBridge {
 
 	function wrapErrorMarker(marker) {
 		var result = {};
-		Object.keys(marker).forEach(function(key) {
+		objectKeys(marker).forEach(function(key) {
 			var isData = key === '__props' || key === '__cause' || key === '__json';
 			defineOwnValue(result, key, isData ? wrapChild(marker[key], false) : marker[key]);
 		});
@@ -1034,7 +1041,7 @@ export class QuickJsBridge implements RuntimeBridge {
 
 	function wrapOwnKeys(v, inCollection) {
 		var result = {};
-		var keys = Object.keys(v);
+		var keys = objectKeys(v);
 		var collides = false;
 		for (var i = 0; i < keys.length; i++) {
 			var key = keys[i];
@@ -1088,20 +1095,20 @@ export class QuickJsBridge implements RuntimeBridge {
 		}
 		if (v.__isKeyedArray === true) {
 			var keyed = v.__items.map(unwrapFromHost);
-			var pkeys = Object.keys(v.__props);
+			var pkeys = objectKeys(v.__props);
 			for (var p = 0; p < pkeys.length; p++) defineOwnValue(keyed, pkeys[p], unwrapFromHost(v.__props[pkeys[p]]));
 			return keyed;
 		}
 		if (v.__isEscaped === true) {
 			var inner = v.__value;
 			var out = {};
-			var ekeys = Object.keys(inner);
+			var ekeys = objectKeys(inner);
 			for (var e = 0; e < ekeys.length; e++) defineOwnValue(out, ekeys[e], unwrapFromHost(inner[ekeys[e]]));
 			return out;
 		}
-		if (Array.isArray(v)) return v.map(unwrapFromHost);
+		if (isArray(v)) return v.map(unwrapFromHost);
 		var result = {};
-		var keys = Object.keys(v);
+		var keys = objectKeys(v);
 		for (var n = 0; n < keys.length; n++) defineOwnValue(result, keys[n], unwrapFromHost(v[keys[n]]));
 		return result;
 	}
diff --git a/packages/@n8n/expression-runtime/src/runtime/context.ts b/packages/@n8n/expression-runtime/src/runtime/context.ts
index 9b6728fc521a..5123666ee3b6 100644
--- a/packages/@n8n/expression-runtime/src/runtime/context.ts
+++ b/packages/@n8n/expression-runtime/src/runtime/context.ts
@@ -12,6 +12,7 @@ import {
 	throwIfErrorSentinel,
 } from './lazy-proxy';
 import { jmesPath } from './jmespath';
+import { assign, defineProperty } from './intrinsics';
 import { decodeHostValue } from './transfer-codec';
 import { isKeyOf } from './utils';
 import type { BridgeMessage } from '../bridge/bridge-messages';
@@ -117,7 +118,7 @@ export function buildContext(
 	// 
[diff truncated]
```
A good answer says what changes and why in a few lines, is true to the diff (no invented behaviour), and names a real risk or says plainly that it sees none.
