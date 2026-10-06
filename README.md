# respelling-features

Learns OpenType GSUB rules that respell English phonemically as you type,
so a font can show ordinary English text in a phonemic spelling.

The rules are trained on the most common English words. A first pass works
like byte-pair encoding: it builds rules from spellings to intermediate tokens
and from tokens to sounds. A second pass flattens these into independent
"half-rules" and re-optimizes each one. Broadening then widens each rule's
context glyphs into glyph sets. Rules are scored by frequency-weighted
Levenshtein distance from the dictionary pronunciation. A simulation of
HarfBuzz's GSUB application does the scoring, and `makefont --check-font`
checks built fonts against that simulation.

## Demo: Heelee ASCII

`web/` holds a demo page that uses the ASCII respelling font. The font is
DejaVu Sans with the trained rules, respelling in ordinary letters (e.g.
"what day is it" becomes "wuht day iz it"):

    python3 web/serve.py   # http://127.0.0.1:8000/

## Layout

- `src/`: the optimizer library. `src/bin/` holds the command-line tools,
  mainly `secondpasstrial` (training) and `makefont` (writes features and
  checks fonts).
- `res/`: the pronunciation decisions and the ASCII respelling table, plus
  a test font for the shaping tests. The dictionaries and word frequencies
  are downloaded and built there (see Data).
- `working/`: trained rule sets (`rules-*.txt`) and generated features.
- `scripts/`: font building (`build_ascii_font.sh`, `build_font.sh` for a font
  of your own with phonetic glyphs, `make_webfont.sh`).
- `cloud/`: running long training jobs on GCP spot VMs (set `PROJECT`).
- `docs/`: design notes.

## Data

The dictionaries and word frequencies aren't in the repository. Download
them, at the versions the rule sets were trained on, and build the files
derived from them:

    scripts/fetch_data.sh
    scripts/make_data.sh

They come from:

- [CMUdict](https://github.com/cmusphinx/cmudict), the CMU Pronouncing
  Dictionary.
- [ReadLex](https://github.com/Shavian-info/readlex), the Shavian reading
  lexicon. The pronunciations are adapted from it into a Central New Jersey
  American phonemic dictionary (`makephonemic`).
- Word frequencies from the Corpus of Contemporary American English, from
  the free top-5,000 sample at [wordfrequency.info](https://www.wordfrequency.info).
- [wordfreq](https://github.com/rspeer/wordfreq) (Python package), for how
  common contractions are.

## Building

Needs Rust, `fonttools` and HarfBuzz's `hb-shape` on the path, plus DejaVu
Sans for the ASCII font, and the data above.

    cargo build --release
    scripts/build_ascii_font.sh working/rules-phonemic-am-5000-800.txt out.ttf --glue
    scripts/make_webfont.sh out.ttf web/heelee-ascii.woff

Building on your own font: `BASE_FONT=yourfont.otf scripts/build_font.sh
<rules> out.otf`. The font must have the phonetic glyphs the rules use.
