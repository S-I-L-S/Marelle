# Platforms

What Marelle runs on, what is verified, and what is still missing. This page is
the source of truth: the README only claims what a green row here supports.
The policy behind the tiers is [ADR-0002](adr/0002-platform-tiers-and-targets.md).

Last reviewed: 2026-11-27.

## What a tier means

| Tier | Promise | CI |
| --- | --- | --- |
| 1 | Works, and stays working. Bugs are fixed first. | Build, clippy and the full test suite on every pull request. Android builds are inspected for libraries Termux lacks. |
| 2 | Builds and is released. Known to work, with fewer eyes on it. | Build on every pull request, tests where a runner or VM exists. |
| 3 | Best effort. Community maintained. | A scheduled build, allowed to fail. |

Marelle never promises more than Rust does. A target that is Tier 3 in Rust
(no prebuilt standard library) is at most Tier 3 here.

## Matrix

"Today" is what CI proves right now.

| Platform | Target | Goal | Today |
| --- | --- | --- | --- |
| Linux x86_64, glibc | `x86_64-unknown-linux-gnu` | 1 | Build, clippy, tests |
| Linux x86_64, musl (static) | `x86_64-unknown-linux-musl` | 1 | Not verified |
| Linux aarch64, glibc | `aarch64-unknown-linux-gnu` | 1 | Not verified |
| Linux aarch64, musl (static) | `aarch64-unknown-linux-musl` | 1 | Not verified |
| Android aarch64 (Termux) | `aarch64-linux-android` | 1 | Build and binary inspection; released |
| Android armv7 (Termux) | `armv7-linux-androideabi` | 1 | Build and binary inspection; released |
| Android x86_64 (emulators, Chromebooks) | `x86_64-linux-android` | 1 | Not verified |
| Windows x86_64 | `x86_64-pc-windows-msvc` | 1 | Not verified. Known gaps below |
| macOS Apple silicon | `aarch64-apple-darwin` | 1 | Not verified |
| macOS Intel | `x86_64-apple-darwin` | 1 | Not verified |
| Windows ARM64 | `aarch64-pc-windows-msvc` | 2 | Not verified |
| Linux armv7 | `armv7-unknown-linux-gnueabihf` | 2 | Not verified |
| Linux riscv64 | `riscv64gc-unknown-linux-gnu` | 2 | Not verified |
| Linux ppc64le | `powerpc64le-unknown-linux-gnu` | 2 | Not verified |
| Linux s390x | `s390x-unknown-linux-gnu` | 2 | Not verified |
| Linux loongarch64 | `loongarch64-unknown-linux-gnu` | 2 | Not verified |
| FreeBSD x86_64 | `x86_64-unknown-freebsd` | 2 | Not verified |
| NetBSD x86_64 | `x86_64-unknown-netbsd` | 2 | Not verified |
| illumos x86_64 | `x86_64-unknown-illumos` | 2 | Not verified |
| OpenBSD x86_64 | `x86_64-unknown-openbsd` | 3 | Not verified. Rust Tier 3: needs `-Zbuild-std` |
| Haiku x86_64 | `x86_64-unknown-haiku` | 3 | Not verified. Rust Tier 3: needs `-Zbuild-std` |
| Redox x86_64 | `x86_64-unknown-redox` | 3 | Not verified. Rust Tier 3: needs `-Zbuild-std` |
| Windows 7 and 8 | `x86_64-win7-windows-msvc` | 3 | Not verified. Rust Tier 3 |
| Windows XP x86 | `i686-rust9x-windows-msvc` | 3 | Not verified. A minimal Rust9x PE32 5.01 probe links with explicit XP subsystem flags. The full workspace check stops in `ring` because no Windows-target C compiler is configured; Marelle still needs that toolchain, XP SDK/import libraries, and a successful XP VM run. |

The minimum operating system version of each row is the one Rust itself supports
for that target. A row that needs a different floor says so in its notes.

## Known gaps

Found by reading the code on 2026-11-23. Each one is tracked as an issue in the
milestone named in brackets.

### Windows [M1]

- **No config directory.** The global settings directory comes from
  `XDG_CONFIG_HOME` or `HOME`, and Windows sets neither by default. There is no
  `APPDATA` or `USERPROFILE` fallback (`settings.rs`).
- **No state directory.** Saved conversations look for `XDG_STATE_HOME` or
  `HOME` (`chat.rs`). Windows should use `LOCALAPPDATA`.
- **`~` expansion** reads `HOME` only (`tools.rs`).
- **The shell tool starts `sh`** (`tools.rs`). It needs a platform shell, a
  `tools.shell` setting, and a description to the model that names the shell in
  use so that it writes the right syntax.
- **Private files.** Session files get mode 0600 on Unix through `cfg(unix)`
  blocks. On Windows nothing is set, so the profile directory's ACL is all that
  protects them. Marelle never stores keys, so only conversation history is
  affected.
- **Tests** assume Unix paths in places, for instance a `HOME` of
  `/home/someone`.

### macOS [M1]

- No known gap. Settings follow the XDG convention (`~/.config/marelle`), as
  most command line tools there do. Needs a CI run to confirm, and notarization
  for releases.

### Linux musl and aarch64 [M1]

- Static builds need `musl-tools` or `zig cc` for the C parts of `ring`.
  Verified by the experiment in [ADR-0004](adr/0004-tls-on-the-far-edge.md).

### The BSD family, illumos, Haiku [M8]

- `sh` and the XDG paths are fine. What is missing is CI: FreeBSD, NetBSD and
  OpenBSD run in VMs, illumos and Haiku need their own images.
- The sandbox plane (M4) has a different native answer on each: `pledge` and
  `unveil` on OpenBSD, Capsicum on FreeBSD.

### Android [M1]

- x86_64 and i686 targets for emulators and Chromebooks are missing.
- Builds are inspected, but no CI job runs the binary. An emulator smoke test is
  planned.

## How a platform becomes Tier 1

A platform moves up when every item is done:

1. The target builds in CI with `--locked`.
2. `cargo clippy -D warnings` is clean on it.
3. The full test suite passes on it, on a native runner or a VM.
4. A release artifact is built, checksummed and attached to the release.
5. An installer or a documented install path exists.
6. This page says so.
