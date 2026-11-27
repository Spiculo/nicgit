#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Builds the release images: nicgit-server and nicgit-worker, tagged with the version in Cargo.toml.
# Builds from this repository as committed (HEAD) and the engine at the commit in engine.rev, both taken with
# `git archive`, so nothing uncommitted and nothing of the local checkouts' state gets in (decision N40).
#   ./scripts/build-images.sh                     load into the local Docker as nicgit-server:<version>, ...
#   NICGIT_IMAGE_PREFIX=registry.example/nicgit PUSH=1 ./scripts/build-images.sh   push instead
# Needs Docker with buildx, and a nicbim checkout (NICBIM_HOME, default ../nicbim) that has the pinned commit.
set -e
here="$(cd "$(dirname "$0")/.." && pwd)"
kim="${NICBIM_HOME:-${KIMBIM_HOME:-$here/../nicbim}}"
prefix="${NICGIT_IMAGE_PREFIX:-nicgit}"
# Docker on Windows needs Windows paths, not Git Bash ones.
native() { case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) cygpath -w "$1" ;; *) printf '%s\n' "$1" ;; esac; }

rev="$(grep -E '^[0-9a-f]{40}$' "$here/engine.rev" | head -1)"
[ -n "$rev" ] || { echo "engine.rev has no commit"; exit 1; }
git -C "$kim" cat-file -e "$rev^{commit}" 2>/dev/null \
  || { echo "the nicbim checkout at $kim does not have $rev: fetch it there first"; exit 1; }
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$here/Cargo.toml" | head -1)"
[ -n "$version" ] || { echo "no version in Cargo.toml"; exit 1; }
if [ -n "$(git -C "$here" status --porcelain --untracked-files=no)" ]; then
  echo "this repository has uncommitted changes; images are built from HEAD, so commit them first"
  exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT INT TERM
"$here/scripts/snapshot.sh" "$here" HEAD "$work/nicgit"
"$here/scripts/snapshot.sh" "$kim" "$rev" "$work/nicbim"

if [ "${PUSH:-0}" = 1 ]; then output=--push; else output=--load; fi
for image in server worker; do
  echo "== $prefix-$image:$version (engine $rev)"
  docker buildx build "$output" \
    -f "$(native "$work/nicgit/deploy/images/Dockerfile.$image")" \
    --build-context nicbim="$(native "$work/nicbim")" \
    --build-arg NICGIT_VERSION="$version" --build-arg NICGIT_ENGINE_REV="$rev" \
    -t "$prefix-$image:$version" "$(native "$work/nicgit")"
done
echo "built $prefix-server:$version and $prefix-worker:$version with engine $rev"
