#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 Marelle contributors

set -euo pipefail

: "${MIRROR_URL:?MIRROR_URL is required}"
: "${MIRROR_USERNAME:?MIRROR_USERNAME is required}"
: "${MIRROR_TOKEN:?MIRROR_TOKEN is required}"

askpass="$(mktemp)"
trap 'rm -f "$askpass"' EXIT
cat > "$askpass" <<'ASKPASS'
#!/bin/sh
case "$1" in
  *Username*) printf '%s\n' "${MIRROR_USERNAME:?}" ;;
  *Password*) printf '%s\n' "${MIRROR_TOKEN:?}" ;;
  *) exit 1 ;;
esac
ASKPASS
chmod 700 "$askpass"
export GIT_ASKPASS="$askpass"
export GIT_TERMINAL_PROMPT=0

# Archived refs are snapshots and must never be overwritten by the source or
# removed from a destination. The symbolic origin/HEAD is not a branch either.
git remote set-head origin --delete >/dev/null 2>&1 || true

branch_refspecs=()
while IFS= read -r local_ref; do
  branch="${local_ref#refs/remotes/origin/}"
  case "$branch" in
    archive/*) continue ;;
  esac
  branch_refspecs+=("+$local_ref:refs/heads/$branch")
done < <(git for-each-ref --format='%(refname)' refs/remotes/origin)

tag_refspecs=()
while IFS= read -r local_ref; do
  tag="${local_ref#refs/tags/}"
  case "$tag" in
    archive/*) continue ;;
  esac
  tag_refspecs+=("+$local_ref:refs/tags/$tag")
done < <(git for-each-ref --format='%(refname)' refs/tags)

if ((${#branch_refspecs[@]} + ${#tag_refspecs[@]} > 0)); then
  git push "$MIRROR_URL" "${branch_refspecs[@]}" "${tag_refspecs[@]}"
fi

# Keep archive/* branches as snapshots of pre-mirror history. Prune deleted
# development branches so active refs remain aligned with the canonical repo.
while IFS=$'\t' read -r _ remote_ref; do
  branch="${remote_ref#refs/heads/}"
  case "$branch" in
    archive/*) continue ;;
  esac
  if ! git show-ref --verify --quiet "refs/remotes/origin/$branch"; then
    git push "$MIRROR_URL" ":$remote_ref"
  fi
done < <(git ls-remote --heads "$MIRROR_URL")

while IFS=$'\t' read -r _ remote_ref; do
  tag="${remote_ref#refs/tags/}"
  case "$tag" in
    archive/*) continue ;;
  esac
  if ! git show-ref --verify --quiet "refs/tags/$tag"; then
    git push "$MIRROR_URL" ":$remote_ref"
  fi
done < <(git ls-remote --tags --refs "$MIRROR_URL")
