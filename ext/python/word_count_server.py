# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 Marelle contributors

"""A dependency-free MCP stdio server, also compatible with Python 2.7."""

from __future__ import print_function

import json
import sys


PROTOCOL_VERSION = "2025-11-25"


def send(message):
    # ASCII escaping keeps stdio independent of the active Windows code page.
    payload = json.dumps(message, ensure_ascii=True, separators=(",", ":"))
    sys.stdout.write(payload + "\n")
    sys.stdout.flush()


def response(request_id, result):
    send({"jsonrpc": "2.0", "id": request_id, "result": result})


def rpc_error(request_id, code, message):
    send({
        "jsonrpc": "2.0",
        "id": request_id,
        "error": {"code": code, "message": message},
    })


def text_result(text, is_error=False):
    return {
        "content": [{"type": "text", "text": text}],
        "isError": is_error,
    }


def handle(request):
    request_id = request.get("id")
    method = request.get("method")
    params = request.get("params") or {}

    if not isinstance(method, str):
        if request_id is not None:
            rpc_error(request_id, -32600, "Invalid request")
        return False

    if request_id is None:
        # notifications/initialized and other notifications need no reply.
        return method == "notifications/exit"

    if method == "initialize":
        offered = params.get("protocolVersion")
        if offered != PROTOCOL_VERSION:
            rpc_error(request_id, -32602, "Unsupported protocol version")
        else:
            response(request_id, {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "marelle-word-count", "version": "1.0.0"},
            })
    elif method == "tools/list":
        response(request_id, {
            "tools": [{
                "name": "word_count",
                "description": "Count whitespace-separated words in a string.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"text": {"type": "string"}},
                    "required": ["text"],
                    "additionalProperties": False,
                },
            }],
        })
    elif method == "tools/call":
        if params.get("name") != "word_count":
            response(request_id, text_result("Unknown tool.", True))
            return False
        arguments = params.get("arguments") or {}
        value = arguments.get("text")
        if not isinstance(value, (str, type(u""))):
            response(request_id, text_result("The text argument must be a string.", True))
            return False
        response(request_id, text_result(str(len(value.split()))))
    elif method == "shutdown":
        response(request_id, {})
    else:
        rpc_error(request_id, -32601, "Method not found")
    return False


def main():
    for line in sys.stdin:
        try:
            request = json.loads(line)
        except (TypeError, ValueError):
            rpc_error(None, -32700, "Parse error")
            continue
        if not isinstance(request, dict) or request.get("jsonrpc") != "2.0":
            request_id = request.get("id") if isinstance(request, dict) else None
            rpc_error(request_id, -32600, "Invalid request")
            continue
        if handle(request):
            break


if __name__ == "__main__":
    main()
