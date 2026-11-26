# Contributing to Marelle

Thank you for looking at Marelle. It is a pre-alpha terminal agent, and the easiest contribution to review is a small, focused pull request. For anything bigger than a small fix, please open an issue first and describe the problem before the solution.

Found a security problem? Do not open a public issue. [SECURITY.md](SECURITY.md) says how to report it privately.

## Set up

You need Rust 1.85 or newer and a C compiler with a linker, which the cryptography library `ring` is built with. On Linux that is `build-essential` or its equivalent, on macOS the Xcode command line tools, on Windows the Visual Studio Build Tools with the C++ workload. Marelle needs no system TLS library: it carries its own list of certificate authorities.

```sh
git clone https://github.com/S-I-L-S/marelle
cd marelle
cargo build
cargo run -p marelle-cli -- help
```

## The checks

Run these before you push. CI runs the same.

| Command | What it proves |
| --- | --- |
| `cargo fmt --all -- --check` | The code is formatted |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | There are no lint warnings |
| `cargo test --workspace --locked` | The tests pass |
| `cargo +1.85.0 check --workspace --all-targets --locked` | The oldest supported Rust still builds it |
| `cargo deny check` | The dependencies keep to the policy in [deny.toml](deny.toml) |

The last one needs [cargo-deny](https://github.com/EmbarkStudios/cargo-deny), in the version that CI runs: `cargo install --locked cargo-deny@0.22.1`.

CI also cross-builds the Android binaries for Termux, and fails when one of them links against `libc++_shared`, `libssl`, `libcrypto` or `libz`, which Termux does not provide by default.

## How the code is laid out

| Crate | Role |
| --- | --- |
| `marelle-cli` | The `marelle` command: arguments, the chat, the settings commands |
| `marelle-providers` | Provider presets, the HTTP client, the OpenAI-compatible and Anthropic wire protocols |
| `marelle-config` | Layered configuration with provenance, typed getters, secret redaction |
| `marelle-i18n` | Message catalogs |
| `marelle-core` | Messages, roles, sessions |
| `marelle-testkit` | A mock HTTP server for tests (not published) |

The design choices behind this, such as why redirects are not followed and why a key is a type and not text, are in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Read "Decisions worth knowing" before you change the network code.

## House rules

A test or a lint enforces most of these. Review enforces the rest.

**Tests never touch the network.** Use the mock server in `marelle-testkit`. It serves canned, slow, broken and redirecting answers on localhost.

**Every message a person sees lives in a catalog.** The catalogs are `crates/marelle-i18n/locales/{en,ru,fr,uk}.txt`: plain `key = value` lines, with `{placeholders}` that the program fills in. English is the reference. Add a new key to all four files in the same pull request. The tests fail when a language lacks a key, repeats one or changes a placeholder. If you are not sure of a translation, give your best attempt and say so in the pull request.

**Keys are a type, not text.** An API key lives in a `Secret`, goes out in one header to the address of its own provider, and never reaches a settings file, a log line, an error message or a test snapshot. A setting holds the name of a variable or a file, never the key itself.

**Every setting is described.** A new setting gets an entry in `crates/marelle-cli/src/schema.rs` with a type and a description. `marelle config schema` prints that table, and `config set` uses it to refuse a value of the wrong shape.

**No unsafe code.** The workspace forbids it.

**Paths are not text.** Build them with `Path` and `PathBuf`, never by gluing `/` into a string, and write tests that compare paths the same way. Windows and Unix spell them differently.

**Every Rust, shell and Python file starts with the licence header**, in the comment style of its language:

```rust
// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Marelle contributors
```

**No emoji.** Not in code, messages, documentation, commits or pull requests. Use ASCII, symbols or kaomoji instead.

## Adding an interface language

1. Copy `crates/marelle-i18n/locales/en.txt` to `<code>.txt` and translate every value. Leave the keys and the `{placeholders}` as they are.
2. In `crates/marelle-i18n/src/lib.rs`, add the language to `Locale`, to `Locale::ALL`, and to `code()` and `catalog()`.
3. In every catalog, add the code to the `help.opt.lang` line.
4. Add the language to the Languages section of `README.md` and `README.ru.md`.
5. Run the tests. They check that every key is covered, that none is repeated and that the placeholders match English.

## Commits and pull requests

- One logical change per commit, and one topic per pull request.
- The first line of a commit message is in the imperative, in sentence case, with no prefix and no full stop, and it fits in about 70 characters: `Find the settings file on Windows`. The body says why. The diff already says what.
- Anything a user can notice gets a line in `CHANGELOG.md`, under "Unreleased" at the top. Add the heading if it is missing.
- Change the documentation in the same pull request as the behaviour it describes.
- Fill in the pull request template. It is short.

## Dependencies and licences

Marelle is AGPL-3.0-only, and contributions are accepted under the same licence. A new dependency needs a licence that is compatible with it (MIT, Apache-2.0, BSD, ISC, Zlib and Unicode are fine; ask first about anything else) and an up-to-date `Cargo.lock`. `scripts/third-party-licenses.py` prints the notices that travel with a release.

[deny.toml](deny.toml) holds the part of this that a machine can check: the accepted licences, no OpenSSL, native-tls or aws-lc, crates from crates.io only, and no yanked version or open advisory. CI also runs it every Monday, so a new advisory is noticed while nobody is changing anything. When an advisory does not reach Marelle and there is no update to take, list it in `deny.toml` with the reason.

Code taken from another project comes in only under a compatible licence and with an entry in [NOTICE](NOTICE).
