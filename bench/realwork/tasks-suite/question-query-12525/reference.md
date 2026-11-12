The PR as its author described it (https://github.com/TanStack/query/pull/12525):
Fixes #11639. Alternative to #11644.

vue-query gets its own `VueQueryExecutionOptions` at its boundary with type assertions when passed to core. Not great not terrible.

Its changes (non-test files):
```diff
diff --git a/.changeset/vue-query-execution-options.md b/.changeset/vue-query-execution-options.md
new file mode 100644
index 00000000000..de22e235eae
--- /dev/null
+++ b/.changeset/vue-query-execution-options.md
@@ -0,0 +1,5 @@
+---
+'@tanstack/vue-query': patch
+---
+
+Accept reactive query options in Vue QueryClient execution methods.
diff --git a/packages/vue-query/src/__tests__/queryClient.shared-execution-options.test-d.ts b/packages/vue-query/src/__tests__/queryClient.shared-execution-options.test-d.ts
new file mode 100644
index 00000000000..06701f1afeb
--- /dev/null
+++ b/packages/vue-query/src/__tests__/queryClient.shared-execution-options.test-d.ts
@@ -0,0 +1,107 @@
+import { describe, expectTypeOf, it } from 'vitest'
+import { computed, ref } from 'vue-demi'
+import { skipToken } from '@tanstack/query-core'
+import { QueryClient } from '../queryClient'
+import { queryOptions } from '../queryOptions'
+import { useQuery } from '../useQuery'
+import type { UseQueryOptions } from '../queryOptions'
+
+describe('shared query options', () => {
+  it('should accept annotated reactive options and preserve selected and cached types', () => {
+    const queryClient = new QueryClient()
+    const enabled = ref(true)
+    const options: UseQueryOptions<string, Error, number> = {
+      queryKey: () => ['key', { id: ref(1) }],
+      queryFn: computed(() =>
+        enabled.value ? () => Promise.resolve('value') : skipToken,
+      ),
+      select: (data) => data.length,
+    }
+
+    expectTypeOf(queryClient.query(options)).toEqualTypeOf<Promise<number>>()
+    // oxlint-disable-next-line tanstack-query/no-restricted-syntax -- regression for shared Vue options
+    expectTypeOf(queryClient.fetchQuery(options)).toEqualTypeOf<
+      Promise<string>
+    >()
+    // oxlint-disable-next-line tanstack-query/no-restricted-syntax -- regression for shared Vue options
+    expectTypeOf(queryClient.ensureQueryData(options)).toEqualTypeOf<
+      Promise<string>
+    >()
+    // oxlint-disable-next-line tanstack-query/no-restricted-syntax -- regression for shared Vue options
+    expectTypeOf(queryClient.prefetchQuery(options)).toEqualTypeOf<
+      Promise<void>
+    >()
+  })
+
+  it('should accept annotated options in a root ref', () => {
+    const queryClient = new QueryClient()
+    const options: UseQueryOptions<string> = ref({
+      queryKey: () => ['key'],
+      queryFn: () => Promise.resolve('value'),
+    })
+
+    expectTypeOf(queryClient.query(options)).toEqualTypeOf<Promise<string>>()
+    // oxlint-disable-next-line tanstack-query/no-restricted-syntax -- regression for shared Vue options
+    expectTypeOf(queryClient.fetchQuery(options)).toEqualTypeOf<
+      Promise<string>
+    >()
+    // oxlint-disable-next-line tanstack-query/no-restricted-syntax -- regression for shared Vue options
+    expectTypeOf(queryClient.ensureQueryData(options)).toEqualTypeOf<
+      Promise<string>
+    >()
+    // oxlint-disable-next-line tanstack-query/no-restricted-syntax -- regression for shared Vue options
+    expectTypeOf(queryClient.prefetchQuery(options)).toEqualTypeOf<
+      Promise<void>
+    >()
+  })
+
+  it('should preserve explicit generic arguments', () => {
+    const queryClient = new QueryClient()
+    const options: UseQueryOptions<string, Error, number, string, ['key']> = {
+      queryKey: () => ['key'],
+      queryFn: () => Promise.resolve('value'),
+      select: (data) => data.length,
+    }
+
+    expectTypeOf(
+      queryClient.query<string, Error, number, string, ['key'], never>(options),
+    ).toEqualTypeOf<Promise<number>>()
+    expectTypeOf(
+      // oxlint-disable-next-line tanstack-query/no-restricted-syntax -- regression for shared Vue options
+      queryClient.fetchQuery<string, Error, string, ['key'], never>(options),
+    ).toEqualTypeOf<Promise<string>>()
+    expectTypeOf(
+      // oxlint-disable-next-line tanstack-query/no-restricted-syntax -- regression for shared Vue options
+      queryClient.ensureQueryData<string, Error, string, ['key']>(options),
+    ).toEqualTypeOf<Promise<string>>()
+    expectTypeOf(
+      // oxlint-disable-next-line tanstack-query/no-restricted-syntax -- regression for shared Vue options
+      queryClient.prefetchQuery<string, Error, string, ['key']>(options),
+    ).toEqualTypeOf<Promise<void>>()
+  })
+
+  it('should retain mutable extracted skipToken options', () => {
+    const queryClient = new QueryClient()
+    const options = { queryKey: ['key'], queryFn: skipToken }
+    const sharedOptions: UseQueryOptions<number> = options
+
+    useQuery(options)
+    queryOptions(options)
+
+    expectTypeOf(queryClient.query(sharedOptions)).toEqualTypeOf<
+      Promise<number>
+    >()
+    // oxlint-disable-next-line tanstack-query/no-restricted-syntax -- regression for shared Vue options
+    expectTypeOf(queryClient.fetchQuery(sharedOptions)).toEqualTypeOf<
+      Promise<number>
+    >()
+    // oxlint-disable-next-line tanstack-query/no-restricted-syntax -- regression for shared Vue options
+    expectTypeOf(queryClient.ensureQueryData(sharedOptions)).toEqualTypeOf<
+      Promise<number>
+    >()
+    // oxlint-disable-next-line tanstack-query/no-restricted-syntax -- regression for shared Vue options
+    expectTypeOf(queryClient.prefetchQuery(sharedOptions)).toEqualTypeOf<
+      Promise<void>
+    >()
+  })
+})
diff --git a/packages/vue-query/src/queryClient.ts b/packages/vue-query/src/queryClient.ts
index 2b02bf080b1..1d76f761e55 100644
--- a/packages/vue-query/src/queryClient.ts
+++ b/packages/vue-query/src/queryClient.ts
@@ -5,7 +5,13 @@ import { QueryCache } from './queryCache'
 import { MutationCache } from './mutationCache'
 import type { UseQueryOptions } from './queryOptions'
 import type { Ref } from 'vue-demi'
-import type { MaybeRefDeep, NoUnknown, QueryClientConfig } from './types'
+import type {
+  MaybeRef,
+  MaybeRefDeep,
+  MaybeRefOrGetter,
+  NoUnknown,
+  QueryClientConfig,
+} from './types'
 impor
[diff truncated]
```
A good answer says what changes and why in a few lines, is true to the diff (no invented behaviour), and names a real risk or says plainly that it sees none.
