#!/bin/bash
# Builds the files in res/ that are derived from the third-party data, after
# scripts/fetch_data.sh has downloaded it:
#
# - res/contraction-frequencies.txt (contraction_frequencies.py; needs the
#   wordfreq Python package)
# - res/topwords-pronunciation.txt (filtercmudict)
# - res/topwords-pronunciation-2.txt (make_corrected_dictionary)
# - res/frequency-table.json (makefrequencytable)
# - res/readlex-entries-top5000.json (makeshortreadlex)
# - res/readlex-entries-top-american5000.json (makeshortreadlex_american)
# - res/pronunciations-phonemic-ipa.txt (makephonemic)
#
#   scripts/make_data.sh
#
# Run from the repository root.

set -euo pipefail

PYTHON=${PYTHON:-$HOME/.local/share/fonttools-venv/bin/python}
BIN=${CARGO_TARGET_DIR:-target}/release

cargo build --release --bin filtercmudict --bin make_corrected_dictionary --bin makefrequencytable \
  --bin makeshortreadlex --bin makeshortreadlex_american --bin makephonemic
mkdir -p working

"$PYTHON" scripts/contraction_frequencies.py
"$BIN"/filtercmudict > /dev/null
"$BIN"/make_corrected_dictionary > /dev/null
"$BIN"/makefrequencytable
"$BIN"/makeshortreadlex > /dev/null
"$BIN"/makeshortreadlex_american > /dev/null
"$BIN"/makephonemic > /dev/null
echo "Built the derived files in res/"
