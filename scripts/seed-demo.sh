#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Demo data on a running dev server: user "ana", org "studio", project studio/sample-house with NicBim's M5
# project (two design options) pushed to it. The account comes from `nicgit-server dev-user`, without Google.
set -e
U="${NICGIT_URL:-http://127.0.0.1:3000}"
here="$(cd "$(dirname "$0")/.." && pwd)"
bin="${NICGIT_SERVER_BIN:-$here/target/dev/debug/nicgit-server}"
user() { NICGIT_DEV=1 "$bin" dev-user --slug ana --email ana@example.com --name "Ana Ferreira" "$@"; }
TOKEN=$(user)
post() { curl -s -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' -d "$2" "$U$1" >/dev/null; }
post /api/orgs '{"slug":"studio","display_name":"Studio Ferreira"}'
post /api/projects '{"owner":"studio","slug":"sample-house","description":"Two-storey sample house with two schemes"}'
fx="${NICBIM_HOME:-$here/../nicbim}/build/m5/project/.git"
git --git-dir="$fx" -c credential.helper= push "http://ana:$TOKEN@${U#http://}/studio/sample-house.git" 'refs/heads/*:refs/heads/*'
# main goes back to before NicBim combined Scheme B, so the proposal below has a real conflict to settle.
before=$(git --git-dir="$fx" rev-parse 'main^1')
git --git-dir="$fx" -c credential.helper= push --force "http://ana:$TOKEN@${U#http://}/studio/sample-house.git" "$before:refs/heads/main"
post /api/projects/studio/sample-house/proposals '{"title":"Glazed south wall and moved column","body":"Scheme B glazes the south wall and moves the column 300 mm east.","source":"Scheme%20B","target":"main"}'
echo "seeded. To look around as ana without Google, set this cookie in your browser:"
echo "  nicgit_session=$(user --session)"
