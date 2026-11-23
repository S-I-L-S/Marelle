# 0003 The extension plane: MCP first, then WASM and Lua

Status: Proposed
Date: 2026-11-23

## Context

Marelle should be open to tools written in any language, on every platform it
runs on. The roadmap lists an MCP client, a plugin protocol and skills as step 6.
There are three ways in, with different strengths:

- **MCP** (Model Context Protocol, JSON-RPC 2.0 over stdio or HTTP) is how other
  agents already talk to tools. A large ecosystem of servers exists, in every
  language. Each server is a separate process, and must be built or installed
  for the platform it runs on.
- **WASM** modules run the same on every platform, in a sandbox, without a
  per-platform build. They need a runtime inside Marelle and a small ABI.
- **Lua** is a small embedded language for hooks and custom commands: react to
  an event, rewrite a prompt, add a slash command.

## Decision

Build them in this order, each behind the same permission policy that already
guards the built-in tools.

1. **MCP first (M2).** A client for stdio, then streamable HTTP. Servers are
   configured globally and per project. A project server asks for trust on first
   use. Its tools appear next to the built-in ones, and pass through the
   permission policy by name.
2. **WASM next (M6).** A host for WASM modules with a small JSON ABI. The runtime
   is chosen by a spike. Reach matters more than speed here: an interpreter that
   builds wherever Rust builds, and needs no JIT, is preferred over a faster one
   that excludes a platform. The component model is revisited when a runtime that
   fits supports it.
3. **Lua for hooks (M6).** Embedded, vendored, small. Hooks can read events and
   decide, and cannot reach the network or the file system except through tools.

Rules for all three:

- Tool results are **data**, not instructions. The agent loop never treats a tool
  result as a command, whatever it says.
- Every call has a timeout and an output size limit.
- Tool names are namespaced by their source (`mcp.<server>.<tool>`,
  `wasm.<plugin>.<tool>`) so a policy can name them without surprises.
- Examples live in `ext/` in several languages, and are tested in CI.

## Consequences

- Any language that can print JSON to stdout can carry a Marelle tool on day one.
- Settings gain a section for servers, with the same layering and provenance as
  everything else.
- The permission policy now has to describe tools it did not write. That is a
  good test for its design.
- Prompt injection through tool results becomes the central security topic of
  the plane. It is handled by the rules above and by the sandbox in M4.

## Alternatives

- **A Marelle-only plugin protocol.** More control, no ecosystem. Rejected,
  because MCP already exists and other agents speak it.
- **WASM only.** Elegant and portable, but the ecosystem of ready tools is MCP.
  Rejected as the first step.
- **Dynamic libraries.** ABI trouble on every platform, no sandbox. Rejected.
