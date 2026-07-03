#!/usr/bin/env python3
"""
Ollama tool-call normalizer proxy.

Sits between `pi` and Ollama. Converts text-format tool calls
(JSON in code blocks or bare JSON) into proper OpenAI tool_calls[].

Run:
    python scripts/ollama_tool_fix_proxy.py [--port 11435] [--upstream http://localhost:11434]

Settings: set baseUrl in ~/.pi/agent/models.json to "http://localhost:11435/v1"
"""
from __future__ import annotations
import argparse, http.server, json, re, sys, time, uuid
from typing import Any, Optional
from urllib import request as urllib_request
from urllib.error import URLError

# Regex patterns
_RE_CODE_OBJ = re.compile(r"```(?:json)?\s*\n?(\{.*?\})\s*\n?```", re.DOTALL)
_RE_CODE_ARR = re.compile(r"```(?:json)?\s*\n?(\[.*?\])\s*\n?```", re.DOTALL)
_RE_BARE_OBJ = re.compile(
    r'(\{[^{}]*"name"\s*:\s*"[^"]+"[^{}]*"arguments"\s*:\s*\{.*?\}[^{}]*\})',
    re.DOTALL,
)
_RE_BARE_ARR = re.compile(
    r'(\[\s*\{[^[\]]*"type"\s*:\s*"function"[^[\]]*\}[^[\]]*\])',
    re.DOTALL,
)


def _parse_obj(raw: str, known: set) -> Optional[dict]:
    raw = raw.strip()
    try:
        obj = json.loads(raw)
    except json.JSONDecodeError:
        depth = 0
        end = -1
        for i, ch in enumerate(raw):
            if ch == "{": depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    end = i + 1
                    break
        if end == -1:
            return None
        try:
            obj = json.loads(raw[:end])
        except json.JSONDecodeError:
            return None
    name = obj.get("name") or obj.get("tool") or (obj.get("function") or {}).get("name")
    if not isinstance(name, str) or not name:
        return None
    if known and name not in known:
        return None
    args = obj.get("arguments") or obj.get("parameters") or obj.get("input") or {}
    return {
        "id": "call_" + uuid.uuid4().hex[:8],
        "type": "function",
        "function": {"name": name, "arguments": json.dumps(args, ensure_ascii=False)},
    }


def _parse_arr(raw: str, known: set) -> Optional[list]:
    try:
        arr = json.loads(raw.strip())
    except json.JSONDecodeError:
        return None
    if not isinstance(arr, list):
        return None
    results = []
    for item in arr:
        if not isinstance(item, dict):
            continue
        # Format 1: OpenAI {type:function, function:{name, arguments}}
        fn = item.get("function") or {}
        name = fn.get("name")
        if name:
            raw_args = fn.get("arguments", {})
            args_str = raw_args if isinstance(raw_args, str) else json.dumps(raw_args, ensure_ascii=False)
        else:
            # Format 2: {name, arguments} or {name, parameters}
            name = item.get("name") or item.get("tool")
            if not isinstance(name, str) or not name:
                continue
            raw_args = item.get("arguments") or item.get("parameters") or item.get("input") or {}
            args_str = raw_args if isinstance(raw_args, str) else json.dumps(raw_args, ensure_ascii=False)
        if known and name not in known:
            continue
        results.append({
            "id": item.get("id") or "call_" + uuid.uuid4().hex[:8],
            "type": "function",
            "function": {"name": name, "arguments": args_str},
        })
    return results if results else None


def extract_tool_calls(content: str, known: set) -> Optional[list]:
    for m in _RE_CODE_OBJ.finditer(content):
        tc = _parse_obj(m.group(1), known)
        if tc:
            return [tc]
    for m in _RE_CODE_ARR.finditer(content):
        arr = _parse_arr(m.group(1), known)
        if arr:
            return arr
    for m in _RE_BARE_ARR.finditer(content):
        arr = _parse_arr(m.group(1), known)
        if arr:
            return arr
    results = []
    for m in _RE_BARE_OBJ.finditer(content):
        tc = _parse_obj(m.group(1), known)
        if tc:
            results.append(tc)
    return results if results else None


def patch_response(body: dict, known: set):
    patched = False
    for choice in body.get('choices', []):
        msg = choice.get('message') or choice.get('delta') or {}
        content = msg.get('content')
        if not isinstance(content, str):
            continue
        tcs = extract_tool_calls(content, known)
        if not tcs:
            continue
        msg['tool_calls'] = tcs
        msg['content'] = None
        choice['finish_reason'] = 'tool_calls'
        patched = True
    return body, patched


def build_sse_bytes(body: dict) -> bytes:
    choices = body.get('choices', [])
    msg = choices[0].get('message', {}) if choices else {}
    finish_reason = choices[0].get('finish_reason', 'stop') if choices else 'stop'
    chunk_id = body.get('id', 'chatcmpl-' + uuid.uuid4().hex[:8])
    model = body.get('model', 'unknown')
    created = body.get('created', int(time.time()))

    def sse(delta: dict, fr: Optional[str] = None) -> bytes:
        obj = {'id': chunk_id, 'object': 'chat.completion.chunk',
               'created': created, 'model': model,
               'choices': [{'index': 0, 'delta': delta, 'finish_reason': fr}]}
        return ('data: ' + json.dumps(obj, ensure_ascii=False) + chr(10) + chr(10)).encode()

    parts = []
    tcs = msg.get('tool_calls')
    if tcs:
        first_tcs = [
            {'index': i, 'id': tc['id'], 'type': 'function',
             'function': {'name': tc['function']['name'], 'arguments': ''}}
            for i, tc in enumerate(tcs)
        ]
        parts.append(sse({'role': 'assistant', 'content': None, 'tool_calls': first_tcs}))
        for i, tc in enumerate(tcs):
            fn_args = tc['function']['arguments']
            parts.append(sse({'tool_calls': [{'index': i, 'function': {'arguments': fn_args}}]}))
    else:
        content = msg.get('content') or ''
        parts.append(sse({'role': 'assistant', 'content': ''}))
        if content:
            parts.append(sse({'content': content}))
    parts.append(sse({}, finish_reason))
    parts.append(('data: [DONE]' + chr(10) + chr(10)).encode())
    return b''.join(parts)


class ProxyHandler(http.server.BaseHTTPRequestHandler):
    upstream: str

    def log_message(self, fmt: str, *args: Any) -> None:
        print('[proxy] ' + (fmt % args), file=sys.stderr, flush=True)

    def _forward(self) -> None:
        length = int(self.headers.get('Content-Length', 0))
        raw_body = self.rfile.read(length) if length else b''

        known: set = set()
        req_json: Optional[dict] = None
        is_chat = '/chat/completions' in self.path
        wants_stream = False

        if is_chat and raw_body:
            try:
                req_json = json.loads(raw_body)
                for tool in req_json.get('tools') or []:
                    fn = tool.get('function') or tool
                    n = fn.get('name')
                    if n:
                        known.add(n)
                wants_stream = bool(req_json.get('stream', False))
                req_json['stream'] = False
                raw_body = json.dumps(req_json, ensure_ascii=False).encode()
            except (json.JSONDecodeError, AttributeError):
                pass

        url = self.upstream.rstrip('/') + self.path
        hdrs = {k: v for k, v in self.headers.items()
                if k.lower() not in ('host', 'content-length')}
        hdrs['Content-Length'] = str(len(raw_body))

        try:
            req = urllib_request.Request(url, data=raw_body, headers=hdrs, method=self.command)
            with urllib_request.urlopen(req, timeout=300) as resp:
                resp_body = resp.read()
                status = resp.status
                resp_hdrs = dict(resp.headers)
        except URLError as exc:
            self.send_error(502, 'Upstream error: ' + str(exc))
            return

        if is_chat and resp_body:
            try:
                rj = json.loads(resp_body)
                rj, was_patched = patch_response(rj, known)
                if was_patched:
                    names = []
                    for ch in rj.get('choices', []):
                        for tc in (ch.get('message') or {}).get('tool_calls') or []:
                            names.append(tc['function']['name'])
                    print('[proxy] patched tool_calls: ' + str(names), file=sys.stderr, flush=True)
                if wants_stream:
                    sse_data = build_sse_bytes(rj)
                    self.send_response(status)
                    self.send_header('Content-Type', 'text/event-stream')
                    self.send_header('Cache-Control', 'no-cache')
                    self.send_header('Connection', 'keep-alive')
                    self.send_header('Content-Length', str(len(sse_data)))
                    self.end_headers()
                    self.wfile.write(sse_data)
                    return
                resp_body = json.dumps(rj, ensure_ascii=False).encode()
            except (json.JSONDecodeError, KeyError):
                pass

        self.send_response(status)
        skip = {'transfer-encoding', 'content-encoding', 'content-length'}
        for k, v in resp_hdrs.items():
            if k.lower() not in skip:
                self.send_header(k, v)
        self.send_header('Content-Length', str(len(resp_body)))
        self.end_headers()
        self.wfile.write(resp_body)

    def do_GET(self) -> None:     # noqa: N802
        self._forward()
    def do_POST(self) -> None:    # noqa: N802
        self._forward()
    def do_OPTIONS(self) -> None: # noqa: N802
        self._forward()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=11435)
    parser.add_argument('--upstream', default='http://localhost:11434')
    args = parser.parse_args()
    ProxyHandler.upstream = args.upstream
    server = http.server.ThreadingHTTPServer(('127.0.0.1', args.port), ProxyHandler)
    print('[proxy] ollama-tool-fix :' + str(args.port) + ' -> ' + args.upstream,
          file=sys.stderr, flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print('[proxy] stopped', file=sys.stderr)


if __name__ == "__main__":
    main()
