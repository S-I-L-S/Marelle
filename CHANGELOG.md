# Changelog

All notable changes are listed here, newest first. The project follows [Semantic Versioning](https://semver.org/), and while the version starts with `0.`, anything may change.

## Unreleased

### Added

- Model discovery: the model selector asks the active provider's model catalogue (`/models` on the configured base URL; Anthropic gets `/v1/models`) and merges what it finds with the configured aliases. Providers without the endpoint keep working with manual names; the lookup is best-effort with an eight-second timeout and its failures are announced in the transcript.
- The README now speaks 51 languages besides English: all of Eurasia, from Portugal to Japan, plus the wider world. Every translation keeps the code blocks byte-identical, carries a language switcher at the top, and no emoji anywhere.
- Update check: when an interactive session starts, Marelle asks GitHub and GitLab for a newer version and mentions it once in the conversation, with the release page. Releases are asked for first, then tags; every failure stays silent. Sources and the timeout are settings: `update.check`, `update.github_repo`, `update.gitlab_repo`, `update.timeout_secs`.
- Tool permission answers speak the interface language, and the confirmation offers "Allow always": the category is granted for the rest of the session, without ever lifting a `deny` policy. In the full-screen interface every tool call and its result join the transcript; the line session keeps them on stderr, and the terminal prompt answers `y` once, `a` always.

- Cancelling an answer: in the full-screen interface Ctrl-C stops the request in flight and keeps the session open; it quits only when no answer is running. A question that no tool had acted on goes back into the input to be edited; if tools had already run, their work stays in the conversation and a note closes the turn. What is typed while the model answers is kept instead of lost. A tool that has already started is not interrupted: the cancel takes effect when it finishes. The line-oriented fallback still ends the session on Ctrl-C.

### Changed

- The name a person sees is always `Marelle`: the version line and the error prefixes now use the capital, like the banner and the help do. Command forms such as `marelle chat` stay lowercase.

### Fixed

- Windows: the user-wide settings file is looked for in `%APPDATA%\marelle\config.toml`, after `XDG_CONFIG_HOME` and before `~/.config`, and a `~` in a settings path falls back to `USERPROFILE` when `HOME` is not set. Before, a Windows session without `HOME` had no user-wide settings file at all. `~\` is accepted there as well as `~/`.

### Security

- The `url` crate, which comes in with the HTTP client, is updated from 2.5.8 to 2.5.9. With 2.5.8, calling `Url::origin()` on a deeply nested `blob:` URL overflowed the stack and aborted the program ([RUSTSEC-2026-0592](https://rustsec.org/advisories/RUSTSEC-2026-0592)). Neither Marelle nor the crates in its build that use `url` call that function.

## 0.1.0-alpha.1

The first release that talks to a model.

### Added

- `marelle chat`: one question, a pipe (`-` stands for standard input), or an interactive session with `/help`, `/clear`, `/model`, `/system` and `/exit`.
- Streaming answers from OpenAI-compatible endpoints and the Anthropic Messages API, with optional display of the model's reasoning and of token counts.
- Provider presets: OpenAI, Anthropic, Z.ai (three endpoints), Zhipu BigModel (two), OpenRouter and Ollama. Any field can be overridden and new providers added through settings.
- `marelle providers`, `marelle config show` and `marelle config explain`.
- Network settings: timeouts, proxy, extra certificate authorities, and a guard against sending a key over plain `http`.
- Interface languages: English, Russian, French and Ukrainian.
- Android builds for Termux (`aarch64` and `armv7`) produced by CI, with checksums and third-party licenses in each archive.
- Termux guide in English and Russian.

### Security

- API keys are held in a type that cannot be printed, sent in a sensitive header, removed from server-supplied error text, never followed across redirects, and refused as a setting value.
