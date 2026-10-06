#!/bin/bash
# Builds a font that respells English in ordinary ASCII letters, from a set
# of half-rules and a respelling file (see makefont --respell), on a copy of
# DejaVu Sans renamed "Heelee ASCII" (see make_ascii_base.py).
#
#   scripts/build_ascii_font.sh <rules file> <output.ttf> [makefont options, e.g. --glue]
#
# RESPELLING (default res/ascii-respelling.txt) and DICTIONARY (default
# readlex-phonemic) can be set in the environment. Checks the result against
# the simulation. Run from the repository root.

set -euo pipefail

RULES=$1
OUT=$2
shift 2

PYTHON=${PYTHON:-$HOME/.local/share/fonttools-venv/bin/python}
MAKEFONT=${MAKEFONT:-./target/release/makefont}
RESPELLING=${RESPELLING:-res/ascii-respelling.txt}
DICTIONARY=${DICTIONARY:-readlex-phonemic}
BASE=$(mktemp --suffix=.ttf)
trap 'rm -f "$BASE"' EXIT

info=$($MAKEFONT --rules "$RULES" --dictionary "$DICTIONARY" --respell "$RESPELLING" --features "$OUT.fea" "$@")
echo "$info"
tokens=$(echo "$info" | sed -n 's/.*the font needs syn0\.\.syn\([0-9]*\).*/\1/p')

"$PYTHON" scripts/make_ascii_base.py "$BASE" --syn $((tokens + 1))
fonttools feaLib -o "$OUT" "$OUT.fea" "$BASE"
echo "Built $OUT"

$MAKEFONT --rules "$RULES" --dictionary "$DICTIONARY" --respell "$RESPELLING" --check-font "$OUT" "$@"
