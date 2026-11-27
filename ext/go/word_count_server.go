// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Marelle contributors

// Dependency-free MCP stdio server. Run with: go run word_count_server.go
package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"strings"
)

const protocolVersion = "2025-11-25"

type request struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params"`
}

func send(value interface{}) {
	encoded, err := json.Marshal(value)
	if err != nil {
		fmt.Fprintln(os.Stderr, "could not encode response:", err)
		return
	}
	_, _ = os.Stdout.Write(append(encoded, '\n'))
}

func rpcError(id json.RawMessage, code int, message string) {
	if len(id) == 0 {
		id = json.RawMessage("null")
	}
	send(map[string]interface{}{
		"jsonrpc": "2.0",
		"id":      id,
		"error":   map[string]interface{}{"code": code, "message": message},
	})
}

func reply(id json.RawMessage, result interface{}) {
	send(map[string]interface{}{"jsonrpc": "2.0", "id": id, "result": result})
}

func textResult(text string, isError bool) map[string]interface{} {
	return map[string]interface{}{
		"content": []interface{}{map[string]interface{}{"type": "text", "text": text}},
		"isError": isError,
	}
}

func handle(line []byte) bool {
	var message request
	if err := json.Unmarshal(line, &message); err != nil {
		rpcError(nil, -32700, "Parse error")
		return false
	}
	if message.JSONRPC != "2.0" || message.Method == "" {
		rpcError(message.ID, -32600, "Invalid request")
		return false
	}
	if len(message.ID) == 0 {
		return message.Method == "notifications/exit"
	}

	params := map[string]json.RawMessage{}
	if len(message.Params) != 0 {
		if err := json.Unmarshal(message.Params, &params); err != nil || params == nil {
			rpcError(message.ID, -32602, "Invalid params")
			return false
		}
	}

	switch message.Method {
	case "initialize":
		var offered string
		_ = json.Unmarshal(params["protocolVersion"], &offered)
		if offered != protocolVersion {
			rpcError(message.ID, -32602, "Unsupported protocol version")
			return false
		}
		reply(message.ID, map[string]interface{}{
			"protocolVersion": protocolVersion,
			"capabilities":    map[string]interface{}{"tools": map[string]interface{}{}},
			"serverInfo":      map[string]interface{}{"name": "marelle-word-count-go", "version": "1.0.0"},
		})
	case "tools/list":
		reply(message.ID, map[string]interface{}{
			"tools": []interface{}{map[string]interface{}{
				"name":        "word_count",
				"description": "Count whitespace-separated words in a string.",
				"inputSchema": map[string]interface{}{
					"type":                 "object",
					"properties":           map[string]interface{}{"text": map[string]interface{}{"type": "string"}},
					"required":             []string{"text"},
					"additionalProperties": false,
				},
			}},
		})
	case "tools/call":
		var name string
		_ = json.Unmarshal(params["name"], &name)
		if name != "word_count" {
			reply(message.ID, textResult("Unknown tool.", true))
			return false
		}
		arguments := map[string]json.RawMessage{}
		if err := json.Unmarshal(params["arguments"], &arguments); err != nil || arguments == nil {
			reply(message.ID, textResult("Arguments must be an object containing a string text value.", true))
			return false
		}
		var text string
		if err := json.Unmarshal(arguments["text"], &text); err != nil {
			reply(message.ID, textResult("The text argument must be a string.", true))
			return false
		}
		reply(message.ID, textResult(fmt.Sprint(len(strings.Fields(text))), false))
	case "shutdown":
		reply(message.ID, map[string]interface{}{})
	default:
		rpcError(message.ID, -32601, "Method not found")
	}
	return false
}

func main() {
	reader := bufio.NewScanner(os.Stdin)
	reader.Buffer(make([]byte, 4096), 1<<20)
	for reader.Scan() {
		line := bytes.TrimSuffix(reader.Bytes(), []byte{'\r'})
		if handle(line) {
			return
		}
	}
	if err := reader.Err(); err != nil {
		rpcError(nil, -32700, "Input line exceeds the 1 MiB limit or could not be read")
	}
}
