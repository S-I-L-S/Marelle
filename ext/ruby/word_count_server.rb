# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 Marelle contributors

# Dependency-free MCP stdio server. Uses Ruby's standard JSON library.
require 'json'

PROTOCOL_VERSION = '2025-11-25'

def send_message(message)
  STDOUT.write(JSON.generate(message) + "\n")
  STDOUT.flush
end

def rpc_error(id, code, message)
  send_message({
    'jsonrpc' => '2.0',
    'id' => id,
    'error' => { 'code' => code, 'message' => message }
  })
end

def reply(id, result)
  send_message({ 'jsonrpc' => '2.0', 'id' => id, 'result' => result })
end

def text_result(text, is_error = false)
  {
    'content' => [{ 'type' => 'text', 'text' => text }],
    'isError' => is_error
  }
end

def handle_message(request)
  unless request.is_a?(Hash) && request['jsonrpc'] == '2.0' && request['method'].is_a?(String)
    id = request.is_a?(Hash) && request.key?('id') ? request['id'] : nil
    rpc_error(id, -32600, 'Invalid request')
    return false
  end

  return request['method'] == 'notifications/exit' unless request.key?('id')

  id = request['id']
  params = request['params'].is_a?(Hash) ? request['params'] : {}
  case request['method']
  when 'initialize'
    unless params['protocolVersion'] == PROTOCOL_VERSION
      rpc_error(id, -32602, 'Unsupported protocol version')
      return false
    end
    reply(id, {
      'protocolVersion' => PROTOCOL_VERSION,
      'capabilities' => { 'tools' => {} },
      'serverInfo' => { 'name' => 'marelle-word-count-ruby', 'version' => '1.0.0' }
    })
  when 'tools/list'
    reply(id, {
      'tools' => [{
        'name' => 'word_count',
        'description' => 'Count whitespace-separated words in a string.',
        'inputSchema' => {
          'type' => 'object',
          'properties' => { 'text' => { 'type' => 'string' } },
          'required' => ['text'],
          'additionalProperties' => false
        }
      }]
    })
  when 'tools/call'
    if params['name'] != 'word_count'
      reply(id, text_result('Unknown tool.', true))
      return false
    end
    arguments = params['arguments'].is_a?(Hash) ? params['arguments'] : {}
    unless arguments['text'].is_a?(String)
      reply(id, text_result('The text argument must be a string.', true))
      return false
    end
    reply(id, text_result(arguments['text'].split.length.to_s))
  when 'shutdown'
    reply(id, {})
  else
    rpc_error(id, -32601, 'Method not found')
  end
  false
end

STDIN.each_line do |line|
  if line.bytesize > 1_048_576
    rpc_error(nil, -32700, 'Input line exceeds the 1 MiB limit')
    next
  end
  begin
    request = JSON.parse(line)
  rescue JSON::ParserError
    rpc_error(nil, -32700, 'Parse error')
    next
  end
  break if handle_message(request)
end
