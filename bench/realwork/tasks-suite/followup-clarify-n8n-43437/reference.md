The PR as its author described it (https://github.com/n8n-io/n8n/pull/43437):
# Description
Backport of #43335 to `release-candidate/2.48.x`.

## Checklist for the author (@looorent) to go through.

- [ ] Review the backport changes
- [ ] Fix possible conflicts
- [ ] Merge to target branch

After this PR has been merged, it will be picked up in the next patch release for release track.

# Original description

## Summary

When a workflow evaluates an expression for each input item, the expression runtime prepares a context for each evaluation. This context provides the current item's data and built-in values such as `$now` and `$today`.

This PR:
- remembers valid timezone names within each runtime instance
- creates each date value only when an expression first reads it

## Why

A large input repeats this setup thousands of times. Luxon, the date library, creates an `Intl.DateTimeFormat` object to validate a timezone name. Repeating that check adds work for a name that the runtime already accepted.

## How this PR relates to #43072 and #43083

Issue #43072 reports that the Postgres node blocks a queue-mode worker for minutes with the `vm` engine. Two separate costs add up in that case:

| Change | What it removes | Where | Effect |
| --- | --- | --- | --- |
| This PR | Repeated setup in each expression evaluation | Expression runtime | Every node that uses the `vm` or `quickjs` engine |
| #43083 (community PR, review tracked in NODE-6370) | The second evaluation of the `queryReplacement` option for each item | Postgres node, Execute Query, v2.5+ | One of the 34 evaluations for each item in case 1 of the issue |

The two changes do not overlap. This PR makes each evaluation cheaper but keeps the number of evaluations. #43083 reduces the number of evaluations but keeps the cost of each one

## Benchmark

Use of `getParameterValue` with the default `America/New_York` timezone. (Case 1 of #43072 at full size: 11,034 items with 40 fields)

| Engine | Time for the loop | Longest event loop block | µs per evaluation |
| --- | ---: | ---: | ---: |
| `vm`, `master` | 40.4 s | 40.4 s | 107.6 |
| `vm`, after the timezone commit | 31.0 s | 31.0 s | 82.5 |
| `vm`, after both commits | 23.0 s | 23.0 s | 61.2 |
| `legacy`, for comparison | 13.6 s | 13.6 s | 36.3 |
| native evaluation, for comparison | 4.8 s | 4.8 s | 12.7 |

## Related Linear tickets, Github issues, and Community forum posts

https://linear.app/n8n/issue/CET-635

Related to #43072. Complementary to #43083.

## Review / Merge checklist

-

Its changes (non-test files):
```diff
diff --git a/packages/@n8n/expression-runtime/src/runtime/context.ts b/packages/@n8n/expression-runtime/src/runtime/context.ts
index f9ef95044ec2..c20bed70db2a 100644
--- a/packages/@n8n/expression-runtime/src/runtime/context.ts
+++ b/packages/@n8n/expression-runtime/src/runtime/context.ts
@@ -50,6 +50,51 @@ const INPUT_RPC_TYPES = {
 } as const satisfies Record<string, BridgeMessage['type']>;
 type InputRpcType = (typeof INPUT_RPC_TYPES)[keyof typeof INPUT_RPC_TYPES];
 
+/**
+ * Creates a check that returns true when a name is a valid IANA timezone.
+ * The check remembers valid names, because luxon builds a new
+ * `Intl.DateTimeFormat` for each validation.
+ */
+function createTimezoneCheck(): (timezone: string) => boolean {
+	const validTimezones = new Set<string>();
+	return (timezone) => {
+		if (!validTimezones.has(timezone) && IANAZone.isValidZone(timezone)) {
+			validTimezones.add(timezone);
+		}
+		return validTimezones.has(timezone);
+	};
+}
+
+const isValidTimezone = createTimezoneCheck();
+
+/**
+ * Defines a property whose value is created on the first read and then kept.
+ * A write replaces the value, as with a plain data property.
+ */
+function defineLazyValue(
+	target: Record<string, unknown>,
+	key: string,
+	create: () => unknown,
+): void {
+	let value: unknown;
+	let isCreated = false;
+	defineProperty(target, key, {
+		get: () => {
+			if (!isCreated) {
+				value = create();
+				isCreated = true;
+			}
+			return value;
+		},
+		set: (newValue: unknown) => {
+			value = newValue;
+			isCreated = true;
+		},
+		enumerable: true,
+		configurable: true,
+	});
+}
+
 // ============================================================================
 // Build Context Function
 // ============================================================================
@@ -138,7 +183,7 @@ export function buildContext(
 	callbacks: BridgeCallbacks,
 	timezone?: string,
 ): Record<string, unknown> {
-	if (timezone && !IANAZone.isValidZone(timezone)) {
+	if (timezone && !isValidTimezone(timezone)) {
 		throw new Error(`Invalid timezone: "${timezone}"`);
 	}
 	Settings.defaultZone = timezone ?? 'system';
@@ -168,8 +213,13 @@ export function buildContext(
 	// already set via Settings.defaultZone above.
 	// -------------------------------------------------------------------------
 
-	target.$now = DateTime.now();
-	target.$today = DateTime.now().set({ hour: 0, minute: 0, second: 0, millisecond: 0 });
+	// The dates are created on first read, because most expressions do not use them.
+	// Keep the zone now: a later evaluation can change Settings.defaultZone before that read.
+	const zone = Settings.defaultZone;
+	defineLazyValue(target, '$now', () => DateTime.now().setZone(zone));
+	defineLazyValue(target, '$today', () =>
+		DateTime.now().setZone(zone).set({ hour: 0, minute: 0, second: 0, millisecond: 0 }),
+	);
 
 	// -------------------------------------------------------------------------
 	// Expose standalone functions (min, max, average, numberList, zip, $ifEmpty, etc.)

```
A good answer says what changes and why in a few lines, is true to the diff (no invented behaviour), and names a real risk or says plainly that it sees none.

For the second message: its premise is half right. They are still created per item (each evaluation builds its own
context), but lazily: on first read instead of when the context is built. A reply that corrects "instead of for every
item" is right. Both are created on first read (an accessor that creates and caches the value), and the
timezone is still taken when the context is built (`Settings.defaultZone` kept in `zone`), so a later change of the
default zone does not leak in. What an existing workflow can see: $now is the time of its first read in that evaluation,
not of the context's setup, a difference of the setup time only (sub-millisecond to milliseconds); within one evaluation
repeated reads return the same value, as before. Writing $now still works (the setter replaces the value). A good second
reply confirms this, stays consistent with the first, and names the first-read timing as the only observable change
rather than inventing others.
