<?php
// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Marelle contributors

// Dependency-free MCP stdio server. Compatible with PHP 5.6 and newer.

const PROTOCOL_VERSION = '2025-11-25';

function send_message($message) {
    $encoded = json_encode($message, JSON_UNESCAPED_SLASHES);
    if ($encoded === false) {
        fwrite(STDERR, "could not encode response\n");
        return;
    }
    fwrite(STDOUT, $encoded . "\n");
    fflush(STDOUT);
}

function rpc_error($id, $code, $message) {
    send_message(array(
        'jsonrpc' => '2.0',
        'id' => $id,
        'error' => array('code' => $code, 'message' => $message)
    ));
}

function reply($id, $result) {
    send_message(array('jsonrpc' => '2.0', 'id' => $id, 'result' => $result));
}

function text_result($text, $is_error) {
    return array(
        'content' => array(array('type' => 'text', 'text' => $text)),
        'isError' => $is_error
    );
}

function handle_message($request) {
    if (!is_array($request) || isset($request['jsonrpc']) !== true ||
        $request['jsonrpc'] !== '2.0' || !isset($request['method']) ||
        !is_string($request['method'])) {
        $id = is_array($request) && array_key_exists('id', $request) ? $request['id'] : null;
        rpc_error($id, -32600, 'Invalid request');
        return false;
    }

    if (!array_key_exists('id', $request)) {
        return $request['method'] === 'notifications/exit';
    }

    $id = $request['id'];
    $params = isset($request['params']) && is_array($request['params']) ? $request['params'] : array();
    switch ($request['method']) {
        case 'initialize':
            if (!isset($params['protocolVersion']) || $params['protocolVersion'] !== PROTOCOL_VERSION) {
                rpc_error($id, -32602, 'Unsupported protocol version');
                break;
            }
            reply($id, array(
                'protocolVersion' => PROTOCOL_VERSION,
                'capabilities' => array('tools' => new stdClass()),
                'serverInfo' => array('name' => 'marelle-word-count-php', 'version' => '1.0.0')
            ));
            break;

        case 'tools/list':
            reply($id, array('tools' => array(array(
                'name' => 'word_count',
                'description' => 'Count whitespace-separated words in a string.',
                'inputSchema' => array(
                    'type' => 'object',
                    'properties' => array('text' => array('type' => 'string')),
                    'required' => array('text'),
                    'additionalProperties' => false
                )
            ))));
            break;

        case 'tools/call':
            if (!isset($params['name']) || $params['name'] !== 'word_count') {
                reply($id, text_result('Unknown tool.', true));
                break;
            }
            $arguments = isset($params['arguments']) && is_array($params['arguments'])
                ? $params['arguments'] : array();
            if (!isset($arguments['text']) || !is_string($arguments['text'])) {
                reply($id, text_result('The text argument must be a string.', true));
                break;
            }
            $words = preg_split('/\s+/u', trim($arguments['text']), -1, PREG_SPLIT_NO_EMPTY);
            if ($words === false) {
                reply($id, text_result('The text is not valid UTF-8.', true));
                break;
            }
            reply($id, text_result((string)count($words), false));
            break;

        case 'shutdown':
            reply($id, new stdClass());
            break;

        default:
            rpc_error($id, -32601, 'Method not found');
    }
    return false;
}

while (($line = fgets(STDIN)) !== false) {
    if (strlen($line) > 1048576) {
        rpc_error(null, -32700, 'Input line exceeds the 1 MiB limit');
        continue;
    }
    $request = json_decode($line, true);
    if (json_last_error() !== JSON_ERROR_NONE) {
        rpc_error(null, -32700, 'Parse error');
        continue;
    }
    if (handle_message($request)) {
        break;
    }
}
