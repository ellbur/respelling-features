#!/bin/bash
# Builds a font from a set of half-rules.
#
#   scripts/build_font.sh <rules file> <output.otf> [makefont options, e.g. --no-merge]
#
# Writes <output>.fea (via makefont), adds the token glyphs and placeholder
# phonetic glyphs to a copy of $BASE_FONT (a CFF font with the phonetic
# glyphs; required), and compiles the features into it. Then checks the
# result against the simulation. Run from the repository root.

set -euo pipefail

RULES=$1
OUT=$2
shift 2

PYTHON=${PYTHON:-$HOME/.local/share/fonttools-venv/bin/python}
MAKEFONT=${MAKEFONT:-./target/release/makefont}
BASE_FONT=${BASE_FONT:?set BASE_FONT to the font to add the features to}
BASE=$(mktemp --suffix=.otf)
trap 'rm -f "$BASE"' EXIT

info=$($MAKEFONT --rules "$RULES" --features "$OUT.fea" "$@")
echo "$info"
tokens=$(echo "$info" | sed -n 's/.*the font needs syn0\.\.syn\([0-9]*\).*/\1/p')
extra=$(echo "$info" | sed -n 's/.*the font needs syn0\.\.syn[0-9]* and \(syn[0-9]*\).*/--empty \1/p')

"$PYTHON" scripts/add_font_glyphs.py "$BASE_FONT" "$BASE" --syn $((tokens + 1)) $extra --placeholder yu:u --placeholder schwa:e
fonttools feaLib -o "$OUT" "$OUT.fea" "$BASE"
echo "Built $OUT"

$MAKEFONT --rules "$RULES" --check-font "$OUT" "$@"
