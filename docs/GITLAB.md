# GitLab mirror and CI

GitHub remains the canonical development repository for now:

- Source: https://github.com/S-I-L-S/marelle
- Active development branch: `feature/native-terminal-ui`

The repository includes a `.gitlab-ci.yml` pipeline so a GitLab copy can run the same core Rust checks. The pipeline runs formatting, Clippy, the workspace test suite, the Rust 1.85 minimum-supported-version check, and a release build. Android/Termux cross-builds remain covered by GitHub Actions because they require the Android NDK setup already maintained in `.github/workflows/ci.yml`.

## Pick how the copy stays in sync

| GitLab tier | Way | Who starts the transfer |
| --- | --- | --- |
| Free | the `Mirror to GitLab` workflow in `.github/workflows/mirror-gitlab.yml` | GitHub pushes to GitLab |
| Premium, Ultimate | pull mirroring in the GitLab project settings | GitLab pulls from GitHub |

GitLab push mirroring is available on every tier, but it pushes out of GitLab, so it cannot keep a copy of a GitHub-canonical repository in sync. Pull mirroring needs Premium or Ultimate. Details: [GitLab documentation, Repository mirroring](https://docs.gitlab.com/user/project/repository/mirror).

Do not create a second independent development history. GitHub is the source of truth; the GitLab project is a read-only copy.

## Create the GitLab project

Create an empty GitLab project named `marelle` under the intended namespace. Do not initialize it with a README, license, or other files, so that the first push cannot clash with an existing history.

## Free tier: push from GitHub Actions

1. In GitLab create an access token with the `write_repository` scope. A personal access token works on every tier; use a project access token where your tier offers them.
2. In GitHub open **Settings → Secrets and variables → Actions**.
3. Add the secret `GITLAB_TOKEN` with the token.
4. Add the variable `GITLAB_PROJECT` with the project path, for example `group/marelle`.
5. Run **Mirror to GitLab** from the Actions tab, or push to any branch.
6. Check that `main` and `feature/native-terminal-ui` appear in GitLab, then open **Build → Pipelines**.

Without the secret and the variable the workflow ends with a notice and the run stays green.

The workflow copies every branch and tag, force-updates them, and prunes refs that no longer exist on GitHub. If the GitLab project already has a history of its own, that history is overwritten, and a protected branch must allow force pushes for the first run to succeed. Prefer a fresh empty project.

## Premium and Ultimate: pull mirror

1. Open **Settings → Repository → Mirroring repositories**.
2. Add `https://github.com/S-I-L-S/marelle.git` as the mirror URL.
3. Select **Pull** as the direction and configure authentication if the GitHub repository is private.
4. Trigger an update and verify that `main` and `feature/native-terminal-ui` appear.
5. Open **Build → Pipelines** and confirm the pipeline passes on the mirrored branch.

## Source-of-truth policy

- Develop and review changes on GitHub.
- Treat GitLab as a read-only mirror unless the project owner explicitly changes this policy.
- Never develop independently on both remotes; divergence can stop mirroring or overwrite refs.
- Do not place API keys or access tokens in repository files, CI variables printed to logs, or remote URLs committed to Git.
