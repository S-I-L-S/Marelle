# Extensions

Marelle's extension plane starts with MCP over stdio. A server is a separate
process, so it can be written in any language with JSON support and a process
runtime on the current machine. The core does not link that language's runtime.

The word-count example is implemented in Python, JavaScript, Go, PHP and Ruby.
Each server uses only its language's standard library and speaks MCP revision
`2025-11-25`. Python supports 2.7 and 3, JavaScript supports Node.js 6 and
newer, and PHP supports 5.6 and newer. The Python server has an end-to-end
lifecycle check; the other examples are not yet covered by CI runtime checks.

| Language | File |
| --- | --- |
| Python | [`python/word_count_server.py`](python/word_count_server.py) |
| JavaScript | [`javascript/word_count_server.js`](javascript/word_count_server.js) |
| Go | [`go/word_count_server.go`](go/word_count_server.go) |
| PHP | [`php/word_count_server.php`](php/word_count_server.php) |
| Ruby | [`ruby/word_count_server.rb`](ruby/word_count_server.rb) |

See [the MCP guide](../docs/MCP.md) for configuration and permissions.
