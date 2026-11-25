## What and why

<!-- What changes, and what problem it solves. Link the issue if there is one: "Closes #123". -->

## How it was checked

<!-- The commands you ran, or what you tried by hand. -->

## Checklist

- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo test --workspace --locked` pass
- [ ] The tests do not need the network
- [ ] Every new message is in all four catalogs (`crates/marelle-i18n/locales`)
- [ ] No key, token or other secret appears in code, tests, logs or messages
- [ ] The documentation and `CHANGELOG.md` (under "Unreleased") are up to date
- [ ] There is no emoji anywhere in the change
