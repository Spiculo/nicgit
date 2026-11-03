#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Demo data on a running dev server: user "ana" (password "correct horse battery"), org "studio",
# project studio/sample-house with KimBim's M5 project (two design options) pushed to it.
set -e
U="${NICGIT_URL:-http://127.0.0.1:3000}"
J=/tmp/nicgit-seed-cookies
rm -f $J
curl -sf -c $J -H 'content-type: application/json' -d '{"slug":"ana","email":"ana@example.com","password":"correct horse battery","display_name":"Ana Ferreira"}' $U/api/auth/signup >/dev/null \
  || curl -sf -c $J -H 'content-type: application/json' -d '{"login":"ana","password":"correct horse battery"}' $U/api/auth/login >/dev/null
curl -s -b $J -H 'content-type: application/json' -d '{"slug":"studio","display_name":"Studio Ferreira"}' $U/api/orgs >/dev/null
curl -s -b $J -H 'content-type: application/json' -d '{"owner":"studio","slug":"sample-house","description":"Two-storey sample house with two schemes"}' $U/api/projects >/dev/null
TOKEN=$(curl -sf -b $J -H 'content-type: application/json' -d '{"name":"seed"}' $U/api/me/tokens | sed 's/.*"token":"\([^"]*\)".*/\1/')
here="$(cd "$(dirname "$0")/.." && pwd)"
fx="${KIMBIM_HOME:-$here/../kimcad}/build/m5/project/.git"
git --git-dir="$fx" -c credential.helper= push "http://ana:$TOKEN@${U#http://}/studio/sample-house.git" 'refs/heads/*:refs/heads/*'
# main goes back to before KimBim combined Scheme B, so the proposal below has a real conflict to settle.
before=$(git --git-dir="$fx" rev-parse 'main^1')
git --git-dir="$fx" -c credential.helper= push --force "http://ana:$TOKEN@${U#http://}/studio/sample-house.git" "$before:refs/heads/main"
curl -s -b $J -H 'content-type: application/json' -d '{"title":"Glazed south wall and moved column","body":"Scheme B glazes the south wall and moves the column 300 mm east.","source":"Scheme%20B","target":"main"}' $U/api/projects/studio/sample-house/proposals >/dev/null
echo "seeded: sign in as ana / correct horse battery"
