The maintainers fixed it in https://github.com/withastro/astro/pull/18430 ("fix(content): compute UTF-8 width arithmetically in getChunkEnd").

Their description:
## Changes

Fixes #18429.

- `getChunkEnd()` in `data-store-writer.ts` called `TextEncoder.encode()` once **per character** of the serialized store, only to learn that character's UTF-8 byte width. With `experimental.collectionStorage: "chunked"` this ran over the whole store on every sync and was ~63s of an 89s content sync on a 44k-entry (~180 MB serialized) store (`node --cpu-prof` on `astro sync`).
- The width is fully determined by the code point, so compute it arithmetically: `< 0x80 → 1`, `< 0x800 → 2`, `< 0x10000 → 3`, else `4`. Lone surrogates fall in the 3-byte branch, which matches the U+FFFD (3 bytes) that `TextEncoder` substitutes for them, so the two agree on every input.
- Output is byte-identical: `endIndex` and `bytes` are unchanged for every input, so existing part files keep their xxhash names and nothing is invalidated.
- Drops the now-unused module-level `ENCODER`.

Real project (16.4k pages, warm cache): content sync **89s → 5.5s**, full `astro build` **2:28 → 1:31**. Standalone: 20 MB string, `chunkString()` 10.30s → 0.08s, identical parts.

## Testing

- Added two unit tests to `chunk-string.test.ts`: code points on each UTF-8 width boundary (U+007F/U+0080/U+07FF/U+0800/U+FFFF/U+10000) with an exact expected split, and lone surrogates (high, low, reversed) budgeted as 3 bytes.
- Existing `chunkString` tests unchanged and pass.
- Differential fuzz of the original vs. the arithmetic `getChunkEnd` on ~25M cases (ASCII, 2/3-byte BMP, boundary values, astral pairs, lone/reversed surrogates, U+FFFD, NUL; `maxBytes` 1–1000; arbitrary start offsets including mid-pair), comparing both return values and the full `chunkString()` split: 0 mismatches. Script and timing repro: https://gist.github.com/Abhirup-99/2adc2e813117342d215f1b998a4a461a
- Verified on the real store that all 14 part files hash identically before and after.

## Docs

No docs change: internal performance fix, no user-facing behaviour change.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

The fix (non-test files):
```diff
diff --git a/.changeset/fast-chunk-end-utf8.md b/.changeset/fast-chunk-end-utf8.md
new file mode 100644
index 000000000000..f90507ca8eb5
--- /dev/null
+++ b/.changeset/fast-chunk-end-utf8.md
@@ -0,0 +1,5 @@
+---
+'astro': patch
+---
+
+Speeds up content sync with `experimental.collectionStorage: "chunked"` by computing each character's UTF-8 width from its code point instead of encoding every character through `TextEncoder`. On a store of ~44k entries this cuts the chunking step from about 60 seconds to well under a second, with byte-identical output.
diff --git a/packages/astro/src/content/data-store-writer.ts b/packages/astro/src/content/data-store-writer.ts
index 9c659a2d8844..93cbd26be51d 100644
--- a/packages/astro/src/content/data-store-writer.ts
+++ b/packages/astro/src/content/data-store-writer.ts
@@ -55,8 +55,6 @@ export function serializeDataStore(collections: Map<string, Map<string, any>>):
 	return devalue.stringify(sortCollections(collections));
 }
 
-const ENCODER = new TextEncoder();
-
 /**
  * Finds the end of the largest UTF-8-safe slice beginning at `startIndex` that
  * fits within `maxBytes`. A code point larger than `maxBytes` is returned on
@@ -68,7 +66,11 @@ function getChunkEnd(str: string, startIndex: number, maxBytes: number) {
 	while (index < str.length) {
 		const codePoint = str.codePointAt(index)!;
 		const charLength = codePoint > 0xffff ? 2 : 1;
-		const charBytes = ENCODER.encode(str.slice(index, index + charLength)).length;
+		// UTF-8 width from the code point. Encoding one character at a time through
+		// TextEncoder gives the same answer but dominates sync time on large stores.
+		// A lone surrogate lands in the 3-byte branch, matching the U+FFFD that
+		// TextEncoder would substitute for it.
+		const charBytes = codePoint < 0x80 ? 1 : codePoint < 0x800 ? 2 : codePoint < 0x10000 ? 3 : 4;
 		if (bytes + charBytes > maxBytes && index > startIndex) {
 			break;
 		}

```
A good answer names this cause and where it is; another real cause, shown in the code, counts too.
