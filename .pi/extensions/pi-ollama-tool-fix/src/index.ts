/**
 * pi-ollama-tool-fix — normalize Ollama text JSON tool calls for pi agent loop.
 *
 * Dev: pi -e P:/opt/docker/pwm-protocol/.pi/extensions/pi-ollama-tool-fix/src/index.ts
 */
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { patchAssistantMessage } from "./patch-message.ts";
import { wrapStreamSimple } from "./stream-wrap.ts";

const CONFIG_PATH = path.join(os.homedir(), ".pi", "agent", "pi-ollama-tool-fix.json");
const LOG_PATH = path.join(os.homedir(), ".pi", "agent", "pi-ollama-tool-fix.log");

interface Config {
  providers: string[];
  logging: boolean;
  /** Drop native OpenAI tools[] from provider payload; parse text JSON instead. Avoids llama.cpp "requires --jinja" 500. */
  stripNativeTools: boolean;
}

const DEFAULT_CONFIG: Config = {
  providers: ["ollama", "default"],
  logging: true,
  stripNativeTools: true,
};

function loadConfig(): Config {
  try {
    if (fs.existsSync(CONFIG_PATH)) {
      return { ...DEFAULT_CONFIG, ...JSON.parse(fs.readFileSync(CONFIG_PATH, "utf8")) };
    }
  } catch {
    /* keep defaults */
  }
  return DEFAULT_CONFIG;
}

function logLine(msg: string, cfg: Config): void {
  if (!cfg.logging) return;
  const line = `[${new Date().toISOString()}] ${msg}\n`;
  try {
    fs.appendFileSync(LOG_PATH, line);
  } catch {
    /* ignore */
  }
}

export default function (pi: ExtensionAPI) {
  const cfg = loadConfig();
  let patchCount = 0;
  let lastPatch: string[] = [];

  const onPatch = (names: string[]) => {
    patchCount += 1;
    lastPatch = names;
    logLine(`patched tool_calls: ${names.join(", ")}`, cfg);
  };

  for (const providerName of cfg.providers) {
    pi.registerProvider(providerName, {
      api: "openai-completions",
      streamSimple: (model, context, options) => wrapStreamSimple(model, context, options, onPatch),
    });
    logLine(`registered streamSimple wrapper on provider=${providerName}`, cfg);
  }

  if (cfg.stripNativeTools) {
    pi.on("before_provider_request", (event) => {
      const payload = event.payload as Record<string, unknown>;
      if ("tools" in payload) {
        delete payload.tools;
        delete payload.tool_choice;
        logLine("stripped native tools from provider payload", cfg);
      }
      return payload;
    });
  }

  // Fallback if stream hook missed (e.g. non-stream path).
  pi.on("message_end", async (event) => {
    const msg = event.message;
    if (!msg || msg.role !== "assistant") return;
    const known = pi.getAllTools().map((t) => t.name);
    const patched = patchAssistantMessage(msg as any, known);
    if (!patched) return;
    onPatch(patched.content.filter((b: any) => b.type === "toolCall").map((b: any) => b.name));
    return { message: patched };
  });

  pi.registerCommand("ollama-tool-fix-status", {
    description: "Show pi-ollama-tool-fix patch stats",
    handler: async (_args, ctx) => {
      const msg = `patches=${patchCount} last=${lastPatch.join(", ") || "(none)"} providers=${cfg.providers.join(",")}`;
      ctx.ui.notify(msg, "info");
      logLine(msg, cfg);
    },
  });
}
