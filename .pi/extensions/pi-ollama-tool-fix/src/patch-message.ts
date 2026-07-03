import type { AssistantMessage, ToolCall } from "@earendil-works/pi-ai";
import {
  collectAssistantText,
  extractToolCalls,
  messageHasToolCalls,
  type ParsedToolCall,
} from "./parse-tool-calls.ts";

let callSeq = 0;

function newToolCallId(): string {
  callSeq += 1;
  return `call_otf_${Date.now()}_${callSeq}`;
}

function toToolCallBlocks(calls: ParsedToolCall[]): ToolCall[] {
  return calls.map((tc) => ({
    type: "toolCall" as const,
    id: newToolCallId(),
    name: tc.name,
    arguments: tc.arguments,
  }));
}

/** Convert text JSON tool calls in an assistant message to native toolCall blocks. */
export function patchAssistantMessage(
  message: AssistantMessage,
  knownTools: Iterable<string>,
): AssistantMessage | null {
  if (messageHasToolCalls(message.content)) return null;

  const text = collectAssistantText(message.content);
  if (!text) return null;

  const calls = extractToolCalls(text, knownTools);
  if (calls.length === 0) return null;

  const nonText = (message.content ?? []).filter(
    (b) => b.type !== "text" || !String((b as { text?: string }).text ?? "").trim(),
  );

  return {
    ...message,
    content: [...nonText, ...toToolCallBlocks(calls)],
    stopReason: "toolUse",
  };
}
