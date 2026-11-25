# SPDX-License-Identifier: AGPL-3.0-or-later
"""The project container format: NicBim writes .nic files (D251).

Older NicBim builds (and the engine commit some NicGit releases pin) called the same format .kim
and shipped ``kimbim.kimfile``. This adapter keeps one import for both, so a worker runs against
either engine during the transition.
"""
from __future__ import annotations

try:  # NicBim current
    from kimbim import nicfile as project_file
except ImportError:  # NicBim before D251
    from kimbim import kimfile as project_file

ProjectFileError = getattr(project_file, "NicError", getattr(project_file, "KimError", Exception))
MANIFEST = project_file.MANIFEST
unpack = project_file.unpack
pack = project_file.pack
remove_tree = project_file.remove_tree
