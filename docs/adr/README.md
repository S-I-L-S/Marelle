# Architecture decision records

A short note for each decision that shapes Marelle: what we faced, what we chose,
what it costs, and what we turned down. They are written once and kept. A
decision that changes gets a new record that supersedes the old one.

| Record | Title | Status |
| --- | --- | --- |
| [0001](0001-polyglot-by-planes.md) | Polyglot by planes | Proposed |
| [0002](0002-platform-tiers-and-targets.md) | Platform tiers and targets | Proposed |
| [0003](0003-extension-plane-mcp-wasm.md) | The extension plane: MCP first, then WASM and Lua | Proposed |
| [0004](0004-tls-on-the-far-edge.md) | TLS on the far edge of the platform list | Proposed, waits for a spike |

## Statuses

- **Proposed.** Written, not yet agreed by the owner.
- **Accepted.** Agreed. Work may rely on it.
- **Superseded by NNNN.** Replaced, kept for the history.

## Template

```markdown
# NNNN Title

Status: Proposed
Date: YYYY-MM-DD

## Context
What forces are at play. What we know, and what we do not.

## Decision
What we do, in the present tense.

## Consequences
What gets easier, what gets harder, what we now have to keep up.

## Alternatives
What else was on the table and why it lost.
```
