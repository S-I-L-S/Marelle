// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Marelle contributors

// Dependency-free MCP stdio server. Uses syntax supported by Node.js 6.
'use strict';

const readline = require('readline');
const PROTOCOL_VERSION = '2025-11-25';

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function send(message) {
  process.stdout.write(JSON.stringify(message) + '\n');
}

function reply(id, result) {
  send({ jsonrpc: '2.0', id: id, result: result });
}

function rpcError(id, code, message) {
  send({ jsonrpc: '2.0', id: id, error: { code: code, message: message } });
}

function textResult(text, isError) {
  return {
    content: [{ type: 'text', text: text }],
    isError: Boolean(isError)
  };
}

function handle(request) {
  if (!isObject(request) || request.jsonrpc !== '2.0' || typeof request.method !== 'string') {
    const id = isObject(request) && Object.prototype.hasOwnProperty.call(request, 'id')
      ? request.id : null;
    rpcError(id, -32600, 'Invalid request');
    return false;
  }

  if (!Object.prototype.hasOwnProperty.call(request, 'id')) {
    return request.method === 'notifications/exit';
  }

  const id = request.id;
  const params = isObject(request.params) ? request.params : {};
  if (request.method === 'initialize') {
    if (params.protocolVersion !== PROTOCOL_VERSION) {
      rpcError(id, -32602, 'Unsupported protocol version');
    } else {
      reply(id, {
        protocolVersion: PROTOCOL_VERSION,
        capabilities: { tools: {} },
        serverInfo: { name: 'marelle-word-count-js', version: '1.0.0' }
      });
    }
  } else if (request.method === 'tools/list') {
    reply(id, {
      tools: [{
        name: 'word_count',
        description: 'Count whitespace-separated words in a string.',
        inputSchema: {
          type: 'object',
          properties: { text: { type: 'string' } },
          required: ['text'],
          additionalProperties: false
        }
      }]
    });
  } else if (request.method === 'tools/call') {
    if (params.name !== 'word_count') {
      reply(id, textResult('Unknown tool.', true));
    } else if (!isObject(params.arguments) || typeof params.arguments.text !== 'string') {
      reply(id, textResult('The text argument must be a string.', true));
    } else {
      const text = params.arguments.text.trim();
      const count = text.length === 0 ? 0 : text.split(/\s+/).length;
      reply(id, textResult(String(count), false));
    }
  } else if (request.method === 'shutdown') {
    reply(id, {});
  } else {
    rpcError(id, -32601, 'Method not found');
  }
  return false;
}

const input = readline.createInterface({ input: process.stdin });
input.on('line', function (line) {
  if (line.endsWith('\r')) {
    line = line.slice(0, -1);
  }
  if (Buffer.byteLength(line, 'utf8') > 1048576) {
    rpcError(null, -32700, 'Input line exceeds the 1 MiB limit');
    return;
  }
  let request;
  try {
    request = JSON.parse(line);
  } catch (error) {
    rpcError(null, -32700, 'Parse error');
    return;
  }
  if (handle(request)) {
    input.close();
  }
});
