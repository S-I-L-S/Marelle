# 0001 Polyglot by planes

Status: Proposed
Date: 2026-11-23

## Context

The goal is a powerful tool built from more than one language, running on many
platforms, with a niche and professional stack wherever that is the better
choice. Quality comes before show.

A single language is the best home for the hot path: the agent loop, the provider
clients, the terminal UI, the settings. It gives one build, one set of tests and
one binary that is small enough for a phone. Around that path sit jobs that have
natural homes elsewhere: tools that other people write in their own languages,
long-lived connections to chat services, native shells for phones and editors,
installers, build automation.

Languages mixed without structure turn into a build matrix that nobody can
maintain. The risk is not that we use many languages. The risk is that we use
them without contracts.

## Decision

The core is Rust. Every other language enters through a **plane**, behind a
**contract**, and never by reaching into the core's internals. The planes are
listed in [PLAN.md](../PLAN.md): extension, code intelligence, reach, front ends,
build and release, docs.

A language is admitted to a plane only when it passes five questions:

1. **Fit.** Does it do this job measurably better than Rust would: reach,
   safety, concurrency, ecosystem, expressiveness?
2. **Contract.** Does it talk to the rest through a protocol (MCP, JSON-RPC,
   LSP, ACP) or a generated binding (UniFFI, WIT), not through shared memory
   and hand-written FFI?
3. **Platforms.** Does it build and run on every platform its plane promises?
4. **CI.** Does it have its own CI job, so that a break is seen the day it
   happens?
5. **Upkeep.** Can a new contributor learn to work on it in a day, with a
   reasonable amount of reading?

The first line of code in a plane gets its own record, with the answers.

Where each language stands today:

| Language | Plane | Status |
| --- | --- | --- |
| Rust | Core, build tools (`xtask`) | Accepted in practice |
| sh, PowerShell | Installers | Accepted in practice |
| Python | Scripts (`scripts/`) | Accepted in practice |
| Anything with an MCP library | Extension | Proposed, M2 |
| Anything that compiles to WASM | Extension | Proposed, M6 |
| Lua | Extension, hooks | Proposed, M6 |
| Zig | Build: cross C toolchain through `zig cc` | Proposed, M1 |
| Gleam or Elixir (BEAM) | Reach | Proposed, decided by a spike in M7 |
| Kotlin | Android front end | Proposed, M8 |
| Swift | Apple front end | Proposed, after M8 |
| TypeScript | Editor extensions | Proposed, after M7 |

Considered and set aside: HolyC runs on TempleOS only. It has no toolchain on any
platform we target, so it fails question 3. The goal of "any language" still
holds for the extension plane: if it can speak MCP or compile to WASM, it can
carry a Marelle tool.

## Consequences

- The Cargo workspace stays pure Rust. A contributor can build and test the core
  without installing Zig, Erlang or Kotlin.
- Each plane lives in its own directory, with its own CI job and its own
  README.
- Contracts are versioned and documented, because they are what keeps the
  planes apart.
- We carry the cost of several toolchains in CI. That is why a language must
  pass the five questions before it arrives.

## Alternatives

- **All Rust.** The simplest and fastest to build. It leaves the extension plane
  to Rust authors only, and gives up what BEAM does best for the reach plane.
  Rejected as the whole answer, kept as the answer for the core.
- **In-process polyglot through FFI.** Fast calls, fragile builds, a different ABI
  story on every platform. Rejected.
- **A fleet of services.** Clean boundaries, but a command line tool should
  start in milliseconds and ship as one file. Rejected for the core. The reach
  plane is the one place where a daemon is natural.
