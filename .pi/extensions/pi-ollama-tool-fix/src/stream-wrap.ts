import {
  type Api,
  type AssistantMessageEventStream,
  type Context,
  type Model,
  type SimpleStreamOptions,
  createAssistantMessageEventStream,
  streamSimple,
  streamSimpleOpenAICompletions,
} from "@earendil-works/pi-ai";
import { patchAssistantMessage } from "./patch-message.ts";

function resolveInnerStream(model: Model<Api>) {
  if (model.api === "openai-completions") {
    return streamSimpleOpenAICompletions;
  }
  return streamSimple;
}

function knownFromContext(context: Context): Set<string> {
  return new Set((context.tools ?? []).map((t) => t.name));
}

/** Wrap provider streamSimple: patch text JSON tool calls at stream end. */
export function wrapStreamSimple(
  model: Model<Api>,
  context: Context,
  options?: SimpleStreamOptions,
  onPatch?: (names: string[]) => void,
): AssistantMessageEventStream {
  const inner = resolveInnerStream(model)(model, context, options);
  const outer = createAssistantMessageEventStream();
  const known = knownFromContext(context);

  (async () => {
    try {
      for await (const event of inner) {
        if (event.type === "done") {
          const patched = patchAssistantMessage(event.message, known);
          if (patched) {
            onPatch?.(patched.content.filter((b) => b.type === "toolCall").map((b) => b.name));
            outer.push({ type: "done", reason: "toolUse", message: patched });
            continue;
          }
        }
        outer.push(event);
        if (event.type === "error") break;
      }
      outer.end();
    } catch (err) {
      outer.push({
        type: "error",
        reason: "error",
        error: {
          role: "assistant",
          content: [],
          api: model.api,
          provider: model.provider,
          model: model.id,
          usage: {
            input: 0,
            output: 0,
            cacheRead: 0,
            cacheWrite: 0,
            totalTokens: 0,
            cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
          },
          stopReason: "error",
          errorMessage: err instanceof Error ? err.message : String(err),
          timestamp: Date.now(),
        },
      });
      outer.end();
    }
  })();

  return outer;
}
