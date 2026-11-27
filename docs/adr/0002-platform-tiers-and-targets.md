# 0002 Platform tiers and targets

Status: Proposed
Date: 2026-11-23

## Context

Marelle should run on a very wide set of systems: Linux and Android first, then
Windows and macOS, then the BSD family and the far edge. Today CI builds and
tests on Linux x86_64 and cross-builds for two Android targets. A README that
says "everywhere" without evidence would be a promise we cannot keep.

Not every platform deserves the same effort, and not every platform has a CI
runner. Rust itself already sorts its targets into tiers by how much support
they get.

## Decision

Three tiers, defined in [PLATFORMS.md](../PLATFORMS.md).

- **Tier 1** has the full CI treatment: build, clippy and the whole test suite on
  every pull request, a release artifact, an install path. Linux x86_64 and
  aarch64 (glibc and musl), Android (aarch64, armv7, x86_64), Windows x86_64 and
  macOS (Apple silicon and Intel).
- **Tier 2** builds on every pull request and is released. Tests run where a
  runner or VM exists: Windows ARM64, Linux armv7, riscv64, ppc64le, s390x,
  loongarch64, FreeBSD, NetBSD, illumos.
- **Tier 3** is best effort and community maintained, built on a schedule and
  allowed to fail: OpenBSD, Haiku, Redox, Windows 7 and 8.

Rules:

1. Marelle never promises more than Rust does. A target that is Rust Tier 3 is at
   most Tier 3 here, and builds with `-Zbuild-std`.
2. A platform is only claimed in the README when its row in PLATFORMS.md says so.
3. Minimum OS versions follow Rust's own floor for each target, unless the row
   says otherwise.
4. Release artifacts are named `marelle-v{version}-{target}` with an archive
   suffix suited to the platform, and carry checksums. Windows builds ship as
   `.zip`, the others as `.tar.gz`.
5. Cross builds for glibc targets use Zig as the C toolchain when that gives an
   older glibc floor than the CI image does.
6. The data and config directories follow each platform's habit: XDG on Linux, the
   BSDs, macOS and Android (Termux); `APPDATA` and `LOCALAPPDATA` on Windows.

## Consequences

- The README can be honest, and the work is ordered: Windows and macOS first,
  because they bring the most users.
- CI grows from one runner to a matrix. Windows and macOS runners are slower, and
  the matrix has to stay under a reasonable time.
- Windows is the largest change in code: paths, the shell tool, private-file
  handling. It is tracked as issues in M1.
- BSD CI runs in VMs and is slow, so it starts as Tier 2 for pull requests that
  touch platform code, and nightly for the rest.

## Alternatives

- **Claim everything Rust can target.** Cheap to write, impossible to keep.
  Rejected.
- **Linux and Android only.** Honest and cheap, and too small for the goal.
- **One flat support list.** Simple, but treats a Haiku build like a Linux build.
  Rejected.
