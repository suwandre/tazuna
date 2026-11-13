The PR as its author described it (https://github.com/n8n-io/n8n/pull/43559):
## Summary

AI Agent V3 rebuilds the model's earlier turns from the tool results before it calls the model again. Before this change, that rebuilt turn did not contain the text that the model wrote before its tool calls.

This PR keeps that text. `createEngineRequests` reads the text from the model message and stores it in the action metadata as `modelText`. `buildSteps` then uses it as the content of the rebuilt AI message:

- Parallel Gemini calls: the real text replaces the synthetic `Calling tools: a, b` text. The thought signature stays on the first function call. With no text, the message sends no text part.
- Single calls (all providers that are not Anthropic thinking): the text is kept, not dropped.
- Anthropic thinking: the text block is kept between the thinking block and the `tool_use` blocks, as in the original response.
- Parallel calls that are replayed as one message each (for example OpenAI): the text goes on the first message only.

Chat memory stores the same rebuilt messages, so memory now holds the model's real text.

## Why

The model sees a history that it did not write. It then copies the synthetic `Calling tool(s): …` format into replies to the user, and memory stores and replays that format in later turns.

The synthetic text has no provider requirement. #24982 copied it from an older placeholder (`Calling X with input: …`) when it grouped parallel Gemini calls. #33640 removed that placeholder for single calls but did not replace it with the real text. Gemini accepts a model turn with a text part before the function calls, and it also accepts a turn with no text part. The fix keeps the existing rule of #34797: when the model wrote no text, the content stays empty so that no empty text part is sent.

## How to test

1. Use the workflow from #42075: Chat Trigger (streaming), AI Agent 3.1, Google Vertex Chat Model (`gemini-3.5-flash`), Simple Memory, and two Code Tools.
2. Set the system message to: "Before every tool call, first write one short sentence telling the user what you are about to do, then call the tool(s) in the same turn. Call independent tools in parallel."
3. Send a message that causes parallel tool calls, then one more call.
4. Look at the next LLM requests. The replayed model turn contains the model's own text and no `Calling tools: …` text.

Unit tests: `pnpm --filter @n8n/n8n-nodes-langchain test utils/agent-execution/test/leadingText.test.ts`. These tests do a full round trip (model message → engine requests → reb

Its changes (non-test files):
```diff
diff --git a/packages/@n8n/nodes-langchain/utils/agent-execution/buildSteps.ts b/packages/@n8n/nodes-langchain/utils/agent-execution/buildSteps.ts
index 4b857b10360d..70afaf752bae 100644
--- a/packages/@n8n/nodes-langchain/utils/agent-execution/buildSteps.ts
+++ b/packages/@n8n/nodes-langchain/utils/agent-execution/buildSteps.ts
@@ -8,9 +8,16 @@ import type {
 	ToolCallData,
 	ThinkingContentBlock,
 	RedactedThinkingContentBlock,
+	TextContentBlock,
 	ToolUseContentBlock,
 } from './types';
 
+type AnthropicContentBlock =
+	| ThinkingContentBlock
+	| RedactedThinkingContentBlock
+	| TextContentBlock
+	| ToolUseContentBlock;
+
 /**
  * Provider-specific metadata extracted from tool action metadata
  */
@@ -86,7 +93,7 @@ interface AnthropicToolCall {
 
 /**
  * Builds Anthropic-specific content blocks for thinking mode.
- * Creates an array with the thinking block followed by one tool_use block per tool call.
+ * Creates an array with the thinking block, the model's text, then one tool_use block per tool call.
  *
  * IMPORTANT: The thinking block must come before tool_use in the message.
  * When content is an array, LangChain ignores tool_calls field for Anthropic,
@@ -96,14 +103,16 @@ interface AnthropicToolCall {
  * @param thinkingType - Type of thinking block (thinking or redacted_thinking)
  * @param thinkingSignature - Optional signature for thinking block
  * @param toolCalls - The tool calls that the model made in this turn
- * @returns Array of content blocks with thinking and tool_use
+ * @param modelText - The text the model wrote before the tool calls
+ * @returns Array of content blocks with thinking, text and tool_use
  */
 function buildAnthropicContentBlocks(
 	thinkingContent: string,
 	thinkingType: 'thinking' | 'redacted_thinking',
 	thinkingSignature: string | undefined,
 	toolCalls: AnthropicToolCall[],
-): Array<ThinkingContentBlock | RedactedThinkingContentBlock | ToolUseContentBlock> {
+	modelText: string | undefined,
+): AnthropicContentBlock[] {
 	// Create thinking block with correct field names for Anthropic API
 	const thinkingBlock: ThinkingContentBlock | RedactedThinkingContentBlock =
 		thinkingType === 'thinking'
@@ -126,38 +135,46 @@ function buildAnthropicContentBlocks(
 		input: omit(toolInput, 'tool'),
 	}));
 
-	return [thinkingBlock, ...toolUseBlocks];
+	const textBlocks: TextContentBlock[] = modelText ? [{ type: 'text', text: modelText }] : [];
+
+	return [thinkingBlock, ...textBlocks, ...toolUseBlocks];
 }
 
 /**
  * Builds message content for AI message, handling provider-specific formats.
- * For Anthropic thinking mode, creates content blocks with thinking and tool_use.
- * For other providers, creates simple string content.
+ * For Anthropic thinking mode, creates content blocks with thinking, text and tool_use.
+ * For other providers, uses the model's text, or no content when the model wrote none.
  *
  * @param providerMetadata - Provider-specific metadata
  * @param toolInput - The tool input data
  * @param toolId - The tool call ID
  * @param toolName - The tool name
- * @param nodeName - The node name for fallback string content
- * @returns Message content (string or content blocks array)
+ * @param modelText - The text the model wrote before the tool call
+ * @returns Content blocks for Anthropic thinking mode, otherwise the model's text or an empty array
  */
 function buildMessageContent(
 	providerMetadata: ProviderMetadata,
 	toolInput: IDataObject,
 	toolId: string,
 	toolName: string,
-): null | Array<ThinkingContentBlock | RedactedThinkingContentBlock | ToolUseContentBlock> {
+	modelText: string | undefined,
+): string | AnthropicContentBlock[] {
 	const { thinkingContent, thinkingType, thinkingSignature } = providerMetadata;
 
 	// Anthropic thinking mode: build content blocks
 	// Empty thinking text is valid because the signature carries the reasoning.
 	if (thinkingContent !== undefined && thinkingType) {
-		return buildAnthropicContentBlocks(thinkingContent, thinkingType, thinkingSignature, [
-			{ toolInput, toolId, toolName },
-		]);
+		return buildAnthropicContentBlocks(
+			thinkingContent,
+			thinkingType,
+			thinkingSignature,
+			[{ toolInput, toolId, toolName }],
+			modelText,
+		);
 	}
 
-	return null;
+	// Empty content produces no text part, which Gemini needs when the model wrote no text
+	return modelText ?? [];
 }
 
 function resolveToolName(tool: EngineResult<RequestResponseMetadata>): string {
@@ -225,15 +242,16 @@ function buildGeminiAdditionalKwargs(
 /**
  * Builds an AIMessage for a single tool call, handling provider-specific formats.
  *
- * For Anthropic thinking mode, content is an array of blocks (thinking + tool_use).
+ * For Anthropic thinking mode, content is an array of blocks (thinking + text + tool_use).
  * For Gemini with thought signatures, additional_kwargs carries the signature.
- * For other providers, content is a simple string with tool_calls set.
+ * For other providers, content is the model's text with tool_calls set.
  */
 function buildIndividualAIMessage(
 	toolId: string,
 	toolName: string,
 	toolInput: IDataObject,
 	providerMetadata: ProviderMetadata,
+	modelText: string | undefined,
 ): AIMessage {
 	const toolCall = {
 		id: toolId,
@@ -242,14 +260,15 @@ function buildIndividualAIMessage(
 		type: 'tool_call' as const,
 	};
 
-	const content = buildMessageContent(providerMetadata, toolInput, toolId, toolName);
+	const content = buildMessageContent(providerMetadata, toolInput, toolId, toolName, modelText);
+	const isAnthropicBlocks = Array.isArray(content) && content.length > 0;
 
 	const additionalKwargs: Record<string, unknown> = providerMetadata.thoughtSignature
 		? buildGeminiAdditionalKwargs(
 				[{ id: toolId, name: toolName, args: toolInput }],
 				providerMetadata.thoughtSignature,
-				// Empty content produces no text part, so the function call is the first part
-				content !== null && content.length > 0,
+				// Text content serializes to a text part before
[diff truncated]
```
A good answer says what changes and why in a few lines, is true to the diff (no invented behaviour), and names a real risk or says plainly that it sees none.
