#!/bin/bash
# Downloads the third-party data into res/, at the versions the rule sets
# were trained on, and checks each file's checksum:
#
# - res/cmudict.dict: the CMU Pronouncing Dictionary
#   (https://github.com/cmusphinx/cmudict, BSD-style license).
# - res/readlex.json: ReadLex, the Shavian reading lexicon
#   (https://github.com/Shavian-info/readlex).
# - res/topwords.txt: the top 5,000 word forms of the Corpus of Contemporary
#   American English, from the free sample at https://www.wordfrequency.info
#   (wordFrequency.xlsx, converted by coca_word_forms.py).
#
# Then run scripts/make_data.sh to build the files derived from them.
#
#   scripts/fetch_data.sh
#
# Run from the repository root.

set -euo pipefail

PYTHON=${PYTHON:-python3}
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

fetch() {
  curl --fail --silent --show-error --location --output "$2" "$1"
}

check() {
  if ! echo "$2  $1" | sha256sum --check --quiet; then
    echo "$1 isn't the version the rules were trained on (its source changed?)" >&2
    exit 1
  fi
  echo "Fetched $1"
}

fetch https://raw.githubusercontent.com/cmusphinx/cmudict/7cd8fb5b5a18058688f413e92282eb18815f1956/cmudict.dict res/cmudict.dict
check res/cmudict.dict 0922441cdbacd173cd41e56556e0bf8a436f3f5940b801a9ef2644a9ea68b548

fetch https://raw.githubusercontent.com/Shavian-info/readlex/caaf9c0fcf968bf5424545ad6606315dd2b8a86e/readlex.json res/readlex.json
check res/readlex.json 4e5f9ba4c89289ec798eab9804a49e6dac75ee7e7ab32d63e873c8c05811477c

# wordfrequency.info's sample isn't versioned, so only the checksum of the
# converted list can tell if it changed.
fetch https://www.wordfrequency.info/samples/wordFrequency.xlsx "$TMP/wordFrequency.xlsx"
"$PYTHON" -I scripts/coca_word_forms.py "$TMP/wordFrequency.xlsx" res/topwords.txt
check res/topwords.txt 0783d2a9aee209c4b447e634d26c51badce0ccbd306666a7cd4eb4fe8a206148
