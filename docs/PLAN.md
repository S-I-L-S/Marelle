# Marelle plan

Where Marelle is going, and why. This page is alive: when a decision changes the
shape of the project it gets an ADR in [docs/adr](adr/README.md), and this page
links to it. The eight roadmap steps in [ARCHITECTURE.md](ARCHITECTURE.md) stay
as they are. This plan adds the tracks that run beside them.

Written on 2026-11-23, against the branch stack that ends at
`feature/cancel-response`.

## What Marelle is

A configurable AI agent for the terminal. You bring the API keys, use many models
at once, tune every detail of the settings and talk in your own language. It
starts as a CLI that feels at home in Termux, and grows into a tool that runs
wherever a terminal runs and works well on codebases written in many languages.

Three things set it apart:

1. **Settings in detail.** Every behaviour has a documented key. Layers
   (defaults, global file, project file, environment, flags) say where a value
   came from, and every colour is a HEX value.
2. **Many models, many languages.** Several models at once (compare, fall back,
   judge), and interface and replies in many human languages.
3. **Everywhere.** One small binary per platform with the same behaviour on
   Linux, Android (Termux), Windows, macOS, the BSDs and further out.
   [PLATFORMS.md](PLATFORMS.md) says what is verified today and what is not.

## Principles

- **Quality before show.** The most optimal option, not the most impressive one.
  Every choice that shapes the project is written down with its reason.
- **A language earns its place.** A niche language is welcome when it is the best
  fit for a job: platform reach, safety, concurrency, expressiveness. It is
  never decoration. [ADR-0001](adr/0001-polyglot-by-planes.md) holds the test.
- **Visible work.** Small commits, pull requests, and CI on every platform we
  claim. A platform without a green CI job is not supported yet, whatever a
  README hopes.
- **Our own code.** Hermes and OpenCode are references. Code from any other
  project comes in only with attribution in [NOTICE](../NOTICE).
- **Keys are a type, not text.** They never reach a settings file and never
  print. See "Decisions worth knowing" in [ARCHITECTURE.md](ARCHITECTURE.md).
- **No emoji in the repository.** ASCII, symbols and kaomoji.

## Shape: planes

Marelle is polyglot by design, and the design is a set of planes. The core is
Rust. Everything else enters through a plane, behind a contract, and never by
reaching into the core's internals.

| Plane | Job | Languages | Contract | When |
| --- | --- | --- | --- | --- |
| Core | Agent loop, providers, tool host, TUI, settings, sessions | Rust | The Cargo workspace in `crates/` | Exists |
| Extension | Tools, hooks and skills written by anyone | Anything that speaks MCP, anything that compiles to WASM, Lua for hooks | MCP (JSON-RPC 2.0), a small WASM ABI, a Lua API | M2, M6 |
| Code intelligence | Understand code in many languages | tree-sitter grammars (C), language servers (any language) | Library calls, LSP | M3 |
| Reach | Gateways to chat apps, schedules, remote sessions | A BEAM language (Gleam or Elixir) | A local JSON-RPC socket to the core | M7 |
| Front ends | Native shells over the same core | Kotlin (Android), Swift (Apple), TypeScript (editors) | UniFFI bindings, ACP | M8 |
| Build and release | Reproducible artifacts for every platform | Rust (`xtask`), Zig (cross toolchain), sh, PowerShell, Nix, Ruby (Homebrew), PKGBUILD | CI workflows | M1 onward |
| Docs | Reference, guides, translations | Markdown, Rust (`xtask`) | `docs/` | M9 |

Planned layout, created plane by plane as the first line of code lands:

```text
crates/      core plane, the Rust workspace
ext/         extension plane: SDKs and examples (python/, typescript/, go/, zig/, wasm/ ...)
reach/       reach plane: the BEAM gateway
apps/        front ends: android/, apple/, editors/
packaging/   installers and recipes: sh, ps1, nix, brew, aur, rpm, deb, termux
xtask/       release and docs automation, in Rust
docs/        this directory
```

## Where we are

The eight steps of the roadmap, as of this plan:

| Step | State |
| --- | --- |
| 1 Talk | Done. Two wire protocols, SSE streaming and complete responses. |
| 2 Settings and look | Done. Layered TOML, schema, wisteria theme with HEX roles. |
| 3 Many models | Done. Aliases, parallel compare, fallback chains, judge. |
| 4 Act | Done on the branch stack, not yet in `main`. Tool loop, built-in tools, permission policy. Tool turns do not stream yet. |
| 5 Remember | Partly. Named conversations are saved and resumable. SQLite and long-term memory are not there yet. |
| 6 Extend | Not started. |
| 7 Reach | Not started. |
| 8 Languages | Partly. Four interface languages, README in 51. |

The honest platform picture is thinner than the goal. CI builds and tests on
Linux x86_64 only, and cross-builds for Android (aarch64, armv7). Windows is
known not to work yet: the settings and state directories assume `HOME`, and the
shell tool starts `sh`. Details are in [PLATFORMS.md](PLATFORMS.md).

## Tracks

The eight steps say what Marelle can do. Four tracks run beside them.

**Everywhere.** Make the platform promise true, tier by tier
([ADR-0002](adr/0002-platform-tiers-and-targets.md)). Windows and macOS first,
then Linux on every architecture, then the BSD family and the far edge.

**Polyglot.** Open the tool to code in any language through the extension plane
([ADR-0003](adr/0003-extension-plane-mcp-wasm.md)): MCP first, then WASM and Lua.

**Code intelligence.** Make the agent good at codebases that mix languages: a
repository map from tree-sitter, language servers as tools, build and test
detection for every common ecosystem.

**Presentation.** The look of the repository and of the tool: one brand system
(wisteria, the hopscotch grid, Braille), a README that stays in step with its 51
translations, a docs generator, man pages and shell completions.

## Milestones

Each milestone has an exit that can be checked. The order is a default, not a
law: M8 and M9 run alongside the others.

| Milestone | Exit |
| --- | --- |
| M0 Foundation | The branch stack is in `main`. This plan, the ADRs and the backlog exist. CONTRIBUTING exists. Every repository URL is correct. A live test against Z.ai is recorded. |
| M1 Everywhere I | Every Tier 1 target builds in CI. Tests pass on Linux (x86_64, aarch64), Windows x86_64 and macOS arm64. `install.sh` and `install.ps1` work. Releases carry checksums for every Tier 1 target. |
| M2 Extend I | An MCP stdio client. Servers are configured per project and global, with a trust prompt. MCP tools pass through the permission policy. Examples in at least five languages. Tool turns stream. |
| M3 Code intelligence | A repository map for the 30 most used languages. A language-server client that exposes diagnostics, definitions and references as tools. Build and test commands detected for at least 12 ecosystems. |
| M4 Sandbox | The shell tool runs inside an OS sandbox on Linux, macOS and Windows under a documented policy. A denied action is reported, never silent. |
| M5 Remember | Sessions in SQLite with full-text search. Long-term memory under the user's control. Context compaction. |
| M6 Extend II | A WASM plugin host, Lua hooks, skills and subagents. |
| M7 Reach | A gateway daemon with Telegram, scheduled tasks, and an ACP server mode for editors. |
| M8 Everywhere II | BSD family, illumos, RISC-V, LoongArch, s390x and ppc64le builds. Windows ARM64. An Android app shell. |
| M9 Presentation and languages | The brand system, README v2, the docs generator, catalogs for every language of Eurasia, man pages and completions. |

Rough order of dependencies: M0, M1, then M2; M3 and M4 can run in parallel after
M2 starts; M5 before M6; M7 after M6.

## Quality gates

These apply to every pull request, on every platform that has reached its tier:

- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --locked`.
- The minimum supported Rust version (today 1.85) still builds.
- Tests never need the network. `marelle-testkit` serves canned answers.
- A new dependency is checked for licence compatibility (AGPL-3.0-only) and
  recorded in the third-party licence list that ships in every release.
- A change that affects what a user sees adds the message to every interface
  catalog, and updates the docs in the same pull request.
- Release builds are reproducible from a tag and ship with checksums.

## Not now

No graphical interface before M8, no hosted service, no model hosting, and no
key storage inside Marelle. Each of these can be reopened with an ADR.

## Open questions for the owner

1. How the branch stack lands in `main`: one pull request per branch, bottom up,
   or one squash. Recommendation: per branch, bottom up, to keep the history.
2. Which Windows versions we promise. Rust's own floor is the default.
3. Gleam or Elixir for the Reach plane. Decided by a spike in M7.
4. Whether to buy code-signing certificates for Windows and macOS releases.
5. Credentials for the GitLab mirror (see [GITLAB.md](GITLAB.md)).
