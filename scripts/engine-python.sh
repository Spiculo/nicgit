#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Run Python in the KimBim engine's pixi environment (activated, so its DLLs/libraries load) with the engine
# and the NicGit worker importable. KIMBIM_HOME points at a kimcad checkout (default: ../kimcad).
here="$(cd "$(dirname "$0")/.." && pwd)"
kim="${KIMBIM_HOME:-$here/../kimcad}"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) sep=";"; k="$(cygpath -w "$kim/python")"; w="$(cygpath -w "$here/workers/engine")";;
  *) sep=":"; k="$kim/python"; w="$here/workers/engine";;
esac
export PYTHONPATH="$k$sep$w"
exec pixi run --manifest-path "$kim/pixi.toml" --frozen python "$@"
