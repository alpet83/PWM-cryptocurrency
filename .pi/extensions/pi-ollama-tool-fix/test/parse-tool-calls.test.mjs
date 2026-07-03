import assert from "node:assert/strict";
import { test } from "node:test";
import {
  extractJsonCandidates,
  extractToolCalls,
} from "../src/parse-tool-calls.ts";

test("nested session_cmd arguments", () => {
  const text = JSON.stringify({
    name: "text_editor_session_cmd",
    arguments: {
      session_id: "abc",
      op: "replace_range",
      op_args: { start: { line: 2, character: 0 }, end: { line: 2, character: 5 }, new_text_content: "BETA" },
    },
  });
  const known = new Set(["text_editor_session_cmd"]);
  const calls = extractToolCalls(text, known);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].name, "text_editor_session_cmd");
  assert.equal(calls[0].arguments.op, "replace_range");
  assert.deepEqual(calls[0].arguments.op_args.start, { line: 2, character: 0 });
});

test("markdown code block", () => {
  const text = 'Here\n```json\n{"name":"write","arguments":{"path":"a.txt","content":"x"}}\n```';
  const calls = extractToolCalls(text, new Set(["write"]));
  assert.equal(calls.length, 1);
  assert.equal(calls[0].name, "write");
});

test("filters unknown tools when allowlist set", () => {
  const text = '{"name":"unknown_tool","arguments":{}}';
  assert.equal(extractToolCalls(text, new Set(["write"])).length, 0);
  assert.equal(extractToolCalls(text, []).length, 1);
});

test("extractJsonCandidates finds nested object", () => {
  const inner = '{"a":{"b":1}}';
  const cands = extractJsonCandidates(`prefix ${inner} suffix`);
  assert.ok(cands.some((c) => JSON.parse(c).a.b === 1));
});
