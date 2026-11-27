# Repository mirrors and GitLab CI

`S-I-L-S/Marelle` on GitHub is the single source of truth. A GitHub Actions
workflow mirrors its Git branches and tags to three read-only copies:

| Destination | Project |
| --- | --- |
| GitHub personal copy | `cornier-en-branche/Marelle` |
| GitLab organization copy | `s-i-l-s/marelle` |
| GitLab personal copy | `cornier-en-branche/marelle` |

The personal GitLab project has an older scaffold. Its existing history is
preserved on `archive/pre-mirror-2026-11-27`; the mirror workflow keeps every
`archive/*` branch and tag while syncing the active branches and tags from the
canonical repository.

The GitLab organization project must be created as an empty project under the
`s-i-l-s` group before its destination is enabled. Do not initialize it with a
README, license, or other files. The existing group project `S.I.L-S` is a
separate project and is not used as the Marelle mirror.

## Enable the mirrors

In `S-I-L-S/Marelle`, open **Settings → Secrets and variables → Actions** and
add these repository variables and secrets:

| Name | Type | Value |
| --- | --- | --- |
| `MIRROR_GITHUB_REPOSITORY` | Variable | `cornier-en-branche/Marelle` |
| `MIRROR_GITHUB_TOKEN` | Secret | Fine-grained token with Contents and Workflows read/write access to that personal repository |
| `GITLAB_ORGANIZATION_PROJECT` | Variable | `s-i-l-s/marelle` |
| `GITLAB_ORGANIZATION_TOKEN` | Secret | GitLab token with `write_repository` access to the organization project |
| `GITLAB_PERSONAL_PROJECT` | Variable | `cornier-en-branche/marelle` |
| `GITLAB_PERSONAL_TOKEN` | Secret | GitLab token with `write_repository` access to the personal project |

Use separate tokens for each destination and limit each token to the named
project where the service permits it. Never put tokens in repository files or
URLs. The workflow reads credentials from environment variables and does not
print them.

The workflow starts after a branch or tag changes, after a ref is deleted, or
when run manually from the Actions page. It pushes every active branch and tag,
force-updates changed refs, removes deleted active refs, and preserves any
`archive/*` ref on a destination. The workflow attempts every configured
destination in the same run. It does not copy issues, pull requests, merge
requests, releases, settings, or permissions.

The personal GitHub repository is empty, so the first successful push will
create its branches. Either service may protect its default branch from force
pushes; allow the matching mirror token to update that branch before enabling
the destination.

## GitLab CI

Each GitLab copy can run `.gitlab-ci.yml` on mirrored branches. It checks the
Rust workspace formatting, Clippy, tests, the Rust 1.85 minimum supported
version, and a release build. GitHub Actions continues to cover Android and
Termux cross-builds.

## Source-of-truth policy

- Develop and review changes on the canonical GitHub repository.
- Treat all personal and GitLab copies as read-only mirrors.
- Never develop independently on mirror branches; their active refs are
  force-updated from GitHub.
- Keep `archive/*` refs for snapshots that predate mirroring.
