# MCP servers

Marelle can load tools from local MCP servers over stdio. The server can be
written in any language that can run as a child process and speak newline-
delimited JSON-RPC. Small dependency-free word-count examples are included for
five languages:

| Language | Runtime command | Example | Notes |
| --- | --- | --- | --- |
| Python | `python` | [`ext/python/word_count_server.py`](../ext/python/word_count_server.py) | Compatible with Python 2.7 and 3; useful on older Windows hosts. |
| JavaScript | `node` | [`ext/javascript/word_count_server.js`](../ext/javascript/word_count_server.js) | Uses syntax available in Node.js 6 and newer. |
| Go | `go run` | [`ext/go/word_count_server.go`](../ext/go/word_count_server.go) | Uses the Go standard library. |
| PHP | `php` | [`ext/php/word_count_server.php`](../ext/php/word_count_server.php) | PHP 5.6 and newer. |
| Ruby | `ruby` | [`ext/ruby/word_count_server.rb`](../ext/ruby/word_count_server.rb) | Uses Ruby's standard JSON library. |

These are protocol examples, not a promise that every runtime supports every
Marelle platform. The Python 2.7 example is the one intended for legacy Windows
hosts; the other runtimes should be checked against the target OS separately.

## Configure a server

Add a server to the global `config.toml` or the project's
`.marelle/config.toml`:

```toml
[mcp]
timeout_secs = 120

[mcp.servers.word-count]
command = "python"
args = ["ext/python/word_count_server.py"]
env = []
```

To try another language, use its runtime and matching file. For example:

```toml
[mcp.servers.word-count-node]
command = "node"
args = ["ext/javascript/word_count_server.js"]
env = []

[mcp.servers.word-count-go]
command = "go"
args = ["run", "ext/go/word_count_server.go"]
env = []

[mcp.servers.word-count-php]
command = "php"
args = ["ext/php/word_count_server.php"]
env = []

[mcp.servers.word-count-ruby]
command = "ruby"
args = ["ext/ruby/word_count_server.rb"]
env = []
```

`command` is launched directly, without a shell. Relative paths in `args` are
resolved by the server from Marelle's current directory. `env` is a list of
environment variable names whose values may be passed to the server; values
are never written to settings. The child receives a small set of platform
runtime variables and nothing else by default.

With the default `ask` policy, Marelle asks before each process start. Choosing
"Allow always" starts it without another prompt for the rest of the current
conversation. Each tool has its own permission setting and asks before it runs:

```toml
[permissions.mcp_servers]
word-count = "ask" # allow, ask, deny

[permissions.mcp_tools.word-count]
word_count = "ask" # allow, ask, deny
```

An interactive "Allow always" answer lasts for the current conversation only.
An explicit `allow` in settings persists; `deny` cannot be overridden by a
session answer. Project servers use the same checks as global servers.

## Protocol support

The client implements MCP `2025-11-25` over stdio: initialization, tool
discovery (including pagination), and tool calls. Tool names longer than 64
characters after namespacing are skipped so providers with a 64-character
function-name limit can use them. It enforces a 1 MiB message limit, a 32 KiB
tool-result limit, a request timeout from 1 to 3600 seconds (120 by default),
and limits on servers and tools. The server process stays alive across calls
in one model/tool turn and closes when the turn ends. Streamable HTTP,
server-to-client callbacks, and the newer draft revision are not implemented
yet.

The Python implementation is covered by an end-to-end stdio lifecycle check.
The other language files are examples and still need runtime checks in CI.

Tool results are passed back to the model as untrusted data. Tool descriptions
and schemas come from the server and should be reviewed when trusting a server.
The process runs with the current user's permissions; MCP does not sandbox it.
