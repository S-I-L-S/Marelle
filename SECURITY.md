# Security policy

Marelle handles API keys and talks to the network on your behalf, so a security report is welcome and is taken seriously.

## Supported versions

Marelle is pre-alpha. Fixes go into the `main` branch and into the latest release. Older releases do not get them.

## Reporting a vulnerability

Please do not open a public issue for a security problem.

Use GitHub's private vulnerability reporting: open the **Security** tab of the repository and choose **Report a vulnerability**. If that button is missing, open a new issue and choose **Private channel for a security report**. The form asks you to leave the problem itself out. A maintainer will answer there and invite you to a private draft security advisory, where the details go.

A good report says what you found, how to reproduce it (the shortest command line or settings file that shows it), the Marelle version from `marelle --version`, and your system. Please do not include a working API key.

A maintainer will acknowledge the report and keep you informed. We ask for a reasonable time to fix the problem before its details are made public, and we will credit you in the advisory if you want that.

## What we especially want to hear about

- A way to make Marelle send an API key anywhere but to the provider it belongs to: through a redirect, a proxy, plain `http`, an error message, a log line or the output of a command.
- A key or token that `marelle config show`, `config explain`, `providers` or an error message prints.
- A weakness in the TLS setup: certificate checking, the built-in list of authorities, or `network.ca_bundle`.
- A flaw in how releases are built, checked or published.

## Handling keys

If you contribute code, the rules for keys are in [CONTRIBUTING.md](CONTRIBUTING.md) and in the architecture notes, [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): a key is a type and not text, it is never stored in a setting, and it never crosses plain `http` to another machine.
