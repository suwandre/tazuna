The maintainers fixed it in https://github.com/withastro/astro/pull/18504 ("Prevent duplicate 1x candidates in Picture source srcset when densities includes 1").

Their description:
## Changes

- `<Picture>` no longer emits a duplicate `1x` candidate in `<source>` `srcset` when `densities` includes `1`. Previously, the component always prepended a plain (descriptorless, implicitly `1x`) `image.src` to the srcset, and then `getSrcSet` would also produce an explicit `1x` entry for the same URL, resulting in invalid markup (`X, X 1x, Y 2x`).
- The plain base candidate is now added only when the srcset does not already contain a `1x` entry. This preserves existing behavior for `densities={[2]}` (the base candidate is still included as the implicit `1x` fallback) while eliminating the duplicate when `densities={[1, 2]}`.

## Testing

- Added a fixture case in `packages/astro/test/core-image.test.ts` asserting that `densities={[1, 2]}` produces exactly `1x, 2x` in the `<source>` srcset with no extra descriptorless candidate. The test fails without the fix and passes with it.

## Docs

- No docs update needed; this is a bug fix for invalid HTML output with no API changes.

Closes #18501

The fix (non-test files):
```diff
diff --git a/.changeset/picture-duplicate-1x-srcset.md b/.changeset/picture-duplicate-1x-srcset.md
new file mode 100644
index 000000000000..a2c61ccd1a30
--- /dev/null
+++ b/.changeset/picture-duplicate-1x-srcset.md
@@ -0,0 +1,5 @@
+---
+'astro': patch
+---
+
+Fixes `<Picture>` emitting a duplicate `1x` candidate in `<source>` `srcset` when `densities` includes `1`
diff --git a/packages/astro/components/Picture.astro b/packages/astro/components/Picture.astro
index 1cbb0989b9c5..95b4c28f2c1d 100644
--- a/packages/astro/components/Picture.astro
+++ b/packages/astro/components/Picture.astro
@@ -135,8 +135,12 @@ const { class: className, ...attributes } = {
 
 <picture {...pictureAttributes}>
 	{Object.entries(optimizedImages).map(([_, image]) => {
+		// The descriptorless `image.src` is the implicit 1x candidate, so it is omitted when
+		// `densities` already produces an explicit 1x one (duplicate densities are invalid HTML).
+		const hasExplicit1x = image.srcSet.values.some((srcSet) => srcSet.descriptor === '1x');
 		const srcsetAttribute =
-			props.densities || (!props.densities && !props.widths && !useResponsive)
+			(props.densities && !hasExplicit1x) ||
+			(!props.densities && !props.widths && !useResponsive)
 				? `${image.src}${image.srcSet.values.length > 0 ? ', ' + image.srcSet.attribute : ''}`
 				: image.srcSet.attribute;
 		return (

```
A good answer names this cause and where it is; another real cause, shown in the code, counts too.
