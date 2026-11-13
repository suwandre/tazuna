The PR as its author described it (https://github.com/withastro/astro/pull/18647):
## Changes

- Replaces the legacy compiler in the TypeScript plugin with `@astrojs/astro2tsx`, which uses compiler-rs to generate virtual TSX and source mappings.
- Declares the Node.js range `^20.19.0 || >=22.12.0` and removes dependencies used only by the legacy converter.

## Testing

- Adds coverage verifying that script and style contents stay out of virtual TSX.
- Existing plugin tests cover Astro references, completions, and implementations through the new converter.

## Docs

Changeset

Made with AI.

Its changes (non-test files):
```diff
diff --git a/.changeset/public-llamas-return.md b/.changeset/public-llamas-return.md
new file mode 100644
index 000000000000..1d93486748f0
--- /dev/null
+++ b/.changeset/public-llamas-return.md
@@ -0,0 +1,5 @@
+---
+'@astrojs/ts-plugin': patch
+---
+
+Fixes TypeScript auto-import suggestions to use the `.astro` component name instead of its generated TSX name.
diff --git a/.changeset/remove-ts-plugin-compiler.md b/.changeset/remove-ts-plugin-compiler.md
new file mode 100644
index 000000000000..b2e0b46f3c80
--- /dev/null
+++ b/.changeset/remove-ts-plugin-compiler.md
@@ -0,0 +1,5 @@
+---
+'@astrojs/ts-plugin': minor
+---
+
+Updates the Node.js version requirement to `^20.19.0 || >=22.12.0`.
diff --git a/packages/language-tools/ts-plugin/package.json b/packages/language-tools/ts-plugin/package.json
index 2c265be3cc2e..876e4eaa956a 100644
--- a/packages/language-tools/ts-plugin/package.json
+++ b/packages/language-tools/ts-plugin/package.json
@@ -27,12 +27,10 @@
   "author": "withastro",
   "license": "MIT",
   "dependencies": {
-    "@astrojs/compiler": "^2.13.1",
+    "@astrojs/astro2tsx": "^0.1.3",
     "@astrojs/yaml2ts": "^0.2.4",
-    "@jridgewell/sourcemap-codec": "^1.6.0",
     "@volar/language-core": "~2.4.30",
-    "@volar/typescript": "~2.4.30",
-    "vscode-languageserver-textdocument": "^1.0.16"
+    "@volar/typescript": "~2.4.30"
   },
   "devDependencies": {
     "@types/mocha": "^10.0.10",
@@ -44,5 +42,8 @@
     "tsx": "^4.23.15",
     "typescript": "^6.0.3",
     "vscode-uri": "^3.2.0"
+  },
+  "engines": {
+    "node": "^20.19.0 || >=22.12.0"
   }
 }
diff --git a/packages/language-tools/ts-plugin/src/astro2tsx.ts b/packages/language-tools/ts-plugin/src/astro2tsx.ts
index f463c3c054f2..3b12ba74910e 100644
--- a/packages/language-tools/ts-plugin/src/astro2tsx.ts
+++ b/packages/language-tools/ts-plugin/src/astro2tsx.ts
@@ -1,158 +1,191 @@
-import path from 'node:path';
-import { convertToTSX } from '@astrojs/compiler/sync';
-import type { ConvertToTSXOptions, TSXResult } from '@astrojs/compiler/types';
-import { decode } from '@jridgewell/sourcemap-codec';
+import { AstroFrontmatterStatus, type ConvertToTsxResult, convertToTsx } from '@astrojs/astro2tsx';
 import type { CodeMapping, VirtualCode } from '@volar/language-core';
-import { TextDocument } from 'vscode-languageserver-textdocument';
 
-function safeConvertToTSX(content: string, options: ConvertToTSXOptions) {
+const SPAN_MAP_KIND_ATOM = 1;
+
+function safeConvertToTSX(content: string, filename: string): ConvertToTsxResult {
 	try {
-		// Match the language server: strip `<script>`/`<style>` content from the TSX.
-		// The compiler otherwise wraps script bodies in `{() => { ... }}`, where
-		// `import` declarations are syntactically invalid and pollute the virtual file.
-		const tsx = convertToTSX(content, {
-			filename: options.filename,
-			includeScripts: false,
-			includeStyles: false,
-		});
-		return tsx;
+		return convertToTsx(content, { filename });
 	} catch (e) {
 		console.error(
-			`There was an error transforming ${options.filename} to TSX. An empty file will be returned instead. Please create an issue: https://github.com/withastro/astro/issues\nError: ${e}.`,
+			`There was an error transforming ${filename} to TSX. An empty file will be returned instead. Please create an issue: https://github.com/withastro/astro/issues\nError: ${e}.`,
 		);
 
 		return {
 			code: '',
-			map: {
-				file: options.filename ?? '',
-				sources: [],
-				sourcesContent: [],
-				names: [],
-				mappings: '',
-				version: 0,
-			},
+			mappings: [],
+			frontmatter: { start: 0, end: 0 },
+			body: { start: 0, end: 0 },
+			frontmatterStatus: AstroFrontmatterStatus.DoesntExist,
+			frontmatterSource: { start: 0, end: 0 },
+			scripts: [],
+			styles: [],
 			diagnostics: [
 				{
-					code: 1000,
-					location: { file: options.filename!, line: 1, column: 1, length: content.length },
+					message:
+						"The Astro compiler encountered an unknown error while transforming this file to TSX. Please create an issue with your code and the error shown in the server's logs: https://github.com/withastro/astro/issues",
 					severity: 1,
-					text: `The Astro compiler encountered an unknown error while parsing this file. Please create an issue with your code and the error shown in the server's logs: https://github.com/withastro/astro/issues`,
+					position: { start: 0, end: content.length },
 				},
 			],
-			metaRanges: {
-				frontmatter: {
-					start: 0,
-					end: 0,
-				},
-				body: {
-					start: 0,
-					end: 0,
-				},
-			},
-		} satisfies TSXResult;
+			hasParseErrors: true,
+		} satisfies ConvertToTsxResult;
 	}
 }
 
+function usePublicComponentName(tsx: ConvertToTsxResult): ConvertToTsxResult {
+	const range = tsx.generatedComponentExport;
+	if (!range) return tsx;
+
+	const exportStatement = tsx.code.slice(range.start, range.end);
+	const exportMatch = /^export \{ (\S+) as (\S+) \};\n$/.exec(exportStatement);
+	if (!exportMatch) return tsx;
+
+	const [, generatedName, publicName] = exportMatch;
+	if (generatedName === publicName) return tsx;
+
+	const defaultExport = `export default function ${generatedName}`;
+	const defaultExportStart = tsx.code.lastIndexOf(defaultExport);
+	if (defaultExportStart === -1) return tsx;
+
+	const replacements = [
+		{
+			start: defaultExportStart + 'export default function '.length,
+			length: generatedName.length,
+			text: publicName,
+		},
+		{
+			start: range.start + exportStatement.indexOf(generatedName),
+			length: generatedName.length,
+			text: publicName,
+		},
+	].sort((a, b) => a.start - b.start);
+
+	let code = tsx.code;
+	for (const replacement of [...replacements].reverse()) {
+		code =
+			code.slice(0, replacement.start) +
+			replacement.text +
+			code.slice(replacement.start + replacement.length);
+	}
+
+	function mapOffset(offset: number) {
+		let shift = 0;
+		for (const replacement of replacements) {
+			const end = replacement.start + replacement.len
[diff truncated]
```
A good answer says what changes and why in a few lines, is true to the diff (no invented behaviour), and names a real risk or says plainly that it sees none.
