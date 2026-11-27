# 0004 TLS on the far edge of the platform list

Status: Proposed, waits for a spike
Date: 2026-11-23

## Context

[ARCHITECTURE.md](../ARCHITECTURE.md) fixes one HTTP stack for every platform:
`reqwest` with `rustls`, the `ring` crypto provider and the Mozilla root list from
`webpki-roots`. It never asks the operating system for certificates. That
decision made Termux work and should stay.

`ring` is not available on every target Rust can build for, and it needs a C
compiler for its assembly and C parts, which a cross build must provide. The
far edge of [PLATFORMS.md](../PLATFORMS.md) (riscv64, ppc64le, s390x, loongarch64,
the BSDs, illumos, Haiku, Redox) may not build with it. We do not yet know which.

## Decision

Run a spike before choosing. For every Tier 2 and Tier 3 target, try
`cargo check` and a build with the current stack, using `zig cc` as the C
compiler for the cross builds, and record the result in PLATFORMS.md.

Then, for the targets that fail, add an alternative crypto provider behind a Cargo
feature on `marelle-providers`:

- `tls-ring` stays the default.
- `tls-aws-lc` or `tls-rustcrypto` is selected only for targets where `ring`
  cannot be built. The choice follows from the spike: what builds, how large the
  binary gets, and what the C toolchain requirement is.

The rest of the design stays: no operating system certificate store, the same
root list, the same redirect and plain-http rules, the same key handling.

## Consequences

- No change for Tier 1 targets.
- A feature flag adds one more build to CI for each alternative provider.
- Binary size on the far edge may grow. It is reported in the release notes.

## Alternatives

- **Switch everything to one other provider.** One path to test, but it gives up
  a working decision on Termux for a benefit only the edge needs.
- **Use the system TLS library on the edge.** Different behaviour on each
  platform, and it brings back the certificate problems that the current design
  avoids.
