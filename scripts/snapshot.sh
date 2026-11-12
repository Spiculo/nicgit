#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copies a repository's files into a folder, for use as a Docker build context:
#   scripts/snapshot.sh <repo> <rev> <dest>     the commit <rev>, nothing else
#   scripts/snapshot.sh <repo> --work <dest>    the working tree as it is: tracked and new files, not ignored ones
# Docker builds from the copy, never from the checkout: Docker on Windows cannot read a build context inside a
# OneDrive folder (its pinned files have an attribute Docker's archiver rejects), and the copy also keeps out
# everything git ignores (target/, node_modules/, data/). The working-tree form uses a temporary index, so the
# repository's own index is untouched.
set -e
repo="$1" what="$2" dest="$3"
[ -n "$repo" ] && [ -n "$what" ] && [ -n "$dest" ] || { echo "usage: snapshot.sh <repo> <rev>|--work <dest>"; exit 2; }
mkdir -p "$dest"
if [ "$what" = --work ]; then
  index="$(mktemp)"
  # Start from the repository's index, so unchanged files need not be hashed again.
  cp "$(git -C "$repo" rev-parse --path-format=absolute --git-path index)" "$index" 2>/dev/null || rm -f "$index"
  tree="$(GIT_INDEX_FILE="$index" git -C "$repo" -c core.safecrlf=false add -A . && GIT_INDEX_FILE="$index" git -C "$repo" write-tree)"
  rm -f "$index"
  git -C "$repo" archive "$tree" | tar -x -C "$dest"
else
  git -C "$repo" archive "$what" | tar -x -C "$dest"
fi
