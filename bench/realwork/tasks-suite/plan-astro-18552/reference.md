The maintainers fixed it in https://github.com/withastro/astro/pull/18552 ("Fix `@astrojs/cloudflare` frontmatter dep scan incorrectly hoisting `import:` object keys").

Their description:
## Changes

- `isImportExpression()` in the Cloudflare adapter's frontmatter dep-scan plugin now also treats a `:` immediately after `import` as "not a static import". This prevents `IMPORT_STMT_RE` from matching object keys like `import: "default"` and incorrectly hoisting them above `async function __astro__()`, which produced invalid JS and caused Rolldown to fail with `Expected 'from' but found ':'`.
- The fix is minimal and safe: an `import` declaration is never followed by a colon, so this exclusion has no false-negative risk.

## Testing

- Added unit tests in `packages/integrations/cloudflare/test/units/wrap-frontmatter.test.ts` covering `import.meta.glob` calls whose options object contains an `import:` key, verifying that `wrapFrontmatter()` leaves the object key in place rather than hoisting it.

## Docs

- No docs update needed; this is a bug fix for an internal dev-only dep-scan heuristic with no user-facing API change.

Closes #18551

The fix (non-test files):
```diff
diff --git a/.changeset/cloudflare-import-object-key.md b/.changeset/cloudflare-import-object-key.md
new file mode 100644
index 000000000000..b21b10236de9
--- /dev/null
+++ b/.changeset/cloudflare-import-object-key.md
@@ -0,0 +1,5 @@
+---
+'@astrojs/cloudflare': patch
+---
+
+Fixes a dev dependency scan error caused by object keys named `import` in frontmatter, such as the `import` option of `import.meta.glob()`
diff --git a/packages/integrations/cloudflare/src/rolldown-plugin-astro-frontmatter.ts b/packages/integrations/cloudflare/src/rolldown-plugin-astro-frontmatter.ts
index 9bb7d74577e4..a8cbca695445 100644
--- a/packages/integrations/cloudflare/src/rolldown-plugin-astro-frontmatter.ts
+++ b/packages/integrations/cloudflare/src/rolldown-plugin-astro-frontmatter.ts
@@ -26,7 +26,7 @@ function isImportExpression(code: string): boolean {
 			index += lineBreak + 3;
 			continue;
 		}
-		return code[index] === '.' || code[index] === '(';
+		return code[index] === '.' || code[index] === '(' || code[index] === ':';
 	}
 	return true;
 }

```
A good answer includes this approach or an equally sound one, says why, and weighs at least one alternative.
