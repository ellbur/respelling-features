#!/bin/bash
# Converts a built font to a WOFF web font.
#
#   scripts/make_webfont.sh <font.otf> <out.woff>
#
# WOFF2 would be smaller but needs the brotli Python module.

set -euo pipefail
PYTHON=${PYTHON:-$HOME/.local/share/fonttools-venv/bin/python}
"$PYTHON" - "$1" "$2" <<'PY'
import sys
from fontTools.ttLib import TTFont
font = TTFont(sys.argv[1])
font.flavor = "woff"
font.save(sys.argv[2])
PY
echo "Wrote $2"
