/** Parse text-format tool calls (Ollama/Qwen) into structured calls. */

export interface ParsedToolCall {
  name: string;
  arguments: Record<string, unknown>;
}

/** Extract balanced JSON object/array substrings from text. */
export function extractJsonCandidates(text: string): string[] {
  const out: string[] = [];
  const trimmed = text.trim();
  if (!trimmed) return out;

  for (let i = 0; i < trimmed.length; i++) {
    const ch = trimmed[i];
    if (ch !== "{" && ch !== "[") continue;
    const end = findBalancedEnd(trimmed, i);
    if (end > i) out.push(trimmed.slice(i, end + 1));
  }
  return out;
}

function findBalancedEnd(text: string, start: number): number {
  const open = text[start];
  const close = open === "{" ? "}" : "]";
  let depth = 0;
  let inString = false;
  let escape = false;

  for (let i = start; i < text.length; i++) {
    const ch = text[i];
    if (inString) {
      if (escape) {
        escape = false;
        continue;
      }
      if (ch === "\\") {
        escape = true;
        continue;
      }
      if (ch === '"') inString = false;
      continue;
    }
    if (ch === '"') {
      inString = true;
      continue;
    }
    if (ch === open) depth++;
    else if (ch === close) {
      depth--;
      if (depth === 0) return i;
    }
  }
  return -1;
}

function normalizeArgs(raw: unknown): Record<string, unknown> {
  if (raw && typeof raw === "object" && !Array.isArray(raw)) {
    return raw as Record<string, unknown>;
  }
  if (typeof raw === "string") {
    try {
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        return parsed as Record<string, unknown>;
      }
    } catch {
      return {};
    }
  }
  return {};
}

function parseOneObject(obj: Record<string, unknown>): ParsedToolCall | null {
  let name: string | undefined;
  let args: Record<string, unknown> = {};

  if (typeof obj.name === "string") {
    name = obj.name;
    args = normalizeArgs(obj.arguments ?? obj.parameters ?? obj.input ?? obj.params);
  } else if (typeof obj.tool === "string") {
    name = obj.tool;
    args = normalizeArgs(obj.input ?? obj.arguments ?? obj.parameters);
  } else {
    const fn = obj.function;
    if (fn && typeof fn === "object" && !Array.isArray(fn)) {
      const f = fn as Record<string, unknown>;
      if (typeof f.name === "string") {
        name = f.name;
        args = normalizeArgs(f.arguments ?? f.input);
      }
    }
  }

  if (!name) return null;
  return { name, arguments: args };
}

function parseArray(arr: unknown[], knownTools: Set<string>): ParsedToolCall[] {
  const out: ParsedToolCall[] = [];
  for (const item of arr) {
    if (!item || typeof item !== "object" || Array.isArray(item)) continue;
    const fn = (item as Record<string, unknown>).function;
    if (fn && typeof fn === "object" && !Array.isArray(fn)) {
      const f = fn as Record<string, unknown>;
      if (typeof f.name === "string") {
        const tc = { name: f.name, arguments: normalizeArgs(f.arguments) };
        if (isAllowed(tc.name, knownTools)) out.push(tc);
      }
      continue;
    }
    const tc = parseOneObject(item as Record<string, unknown>);
    if (tc && isAllowed(tc.name, knownTools)) out.push(tc);
  }
  return out;
}

function isAllowed(name: string, knownTools: Set<string>): boolean {
  return knownTools.size === 0 || knownTools.has(name);
}

/** Parse tool calls from assistant text; filters by knownTools when non-empty. */
export function extractToolCalls(text: string, knownTools: Iterable<string> = []): ParsedToolCall[] {
  const known = new Set(knownTools);
  const seen = new Set<string>();
  const results: ParsedToolCall[] = [];

  const mdRe = /```(?:json)?\s*([\s\S]*?)\s*```/g;
  let m: RegExpExecArray | null;
  const candidates: string[] = [];
  while ((m = mdRe.exec(text)) !== null) candidates.push(m[1]);
  candidates.push(...extractJsonCandidates(text));
  candidates.push(text.trim());

  for (const raw of candidates) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      continue;
    }

    if (Array.isArray(parsed)) {
      for (const tc of parseArray(parsed, known)) {
        const key = `${tc.name}:${JSON.stringify(tc.arguments)}`;
        if (seen.has(key)) continue;
        seen.add(key);
        results.push(tc);
      }
      continue;
    }

    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) continue;
    const tc = parseOneObject(parsed as Record<string, unknown>);
    if (!tc || !isAllowed(tc.name, known)) continue;
    const key = `${tc.name}:${JSON.stringify(tc.arguments)}`;
    if (seen.has(key)) continue;
    seen.add(key);
    results.push(tc);
  }

  return results;
}

export function collectAssistantText(content: unknown): string {
  if (!Array.isArray(content)) return "";
  return content
    .filter((b) => b && typeof b === "object" && (b as { type?: string }).type === "text")
    .map((b) => String((b as { text?: string }).text ?? ""))
    .join("\n")
    .trim();
}

export function messageHasToolCalls(content: unknown): boolean {
  return Array.isArray(content) && content.some((b) => b && (b as { type?: string }).type === "toolCall");
}
