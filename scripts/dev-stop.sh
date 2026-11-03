#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Stop the local dev server and every engine worker started by scripts/dev.sh.
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    taskkill //F //IM nicgit-server.exe >/dev/null 2>&1
    powershell -NoProfile -Command "Get-CimInstance Win32_Process | Where-Object { \$_.CommandLine -like '*nicgit_worker*' -and \$_.Name -like 'python*' } | ForEach-Object { Stop-Process -Id \$_.ProcessId -Force }" ;;
  *)
    pkill -f nicgit-server; pkill -f nicgit_worker ;;
esac
echo stopped
