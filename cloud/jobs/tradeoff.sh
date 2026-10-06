#!/bin/bash
# Shaping time against quality, for kinds of rule set at several sizes. Run
# on a job VM (see cloud/launch.sh):
#
#   cp <base font>.otf working/base.otf
#   cloud/launch.sh tradeoff c2d-highcpu-32 12 -- bash cloud/jobs/tradeoff.sh
#   cloud/launch.sh anchors c2d-highcpu-32 12 -- WORDS=300 MODES=anchors+context \
#     RENDERINGS=contextual OUT=working/tradeoff-anchors bash cloud/jobs/tradeoff.sh
#
# The kinds (MODES, joined with +): anchors (rules may have ^/$ and context),
# antianchors (~ instead of ^/$, and context), both (^/$ and ~, and context),
# broadened (the "both" rule set, then broadened: see
# SecondPass::broaden_sweep), context (context but no ^/$), nocontext
# (neither).
#
# Trains every rule set at once (threads in proportion to size), then, with
# the machine otherwise idle, builds a font from each in each RENDERINGS
# (contextual: merged contextual lookups; plain: merged plain lookups of one
# kind each), checks it against the simulation, and times shaping
# res/shaping-corpus.txt. Results go to $OUT/results.tsv. Finished rule sets
# are kept, so a relaunch (RESUME=...) picks up where it stopped.

set -euo pipefail

WORDS=${WORDS:-1000}
# size:threads
SIZES=${SIZES:-"30:2 50:3 80:4 120:7"}
MODES=${MODES:-context+nocontext}
RENDERINGS=${RENDERINGS:-contextual+plain}
OUT=${OUT:-working/tradeoff}
mkdir -p $OUT

apt-get install -y -q libharfbuzz-bin python3-venv > /dev/null
python3 -m venv /opt/fonttools
/opt/fonttools/bin/pip install -q fonttools
export PYTHON=/opt/fonttools/bin/python PATH=/opt/fonttools/bin:$PATH BASE_FONT=working/base.otf
cargo build --release --bin makefont --bin showerrors

for st in $SIZES; do
  size=${st%:*}
  threads=${st#*:}
  for mode in ${MODES//+/ }; do
    if [ -f $OUT/rules-$mode-$size.txt ]; then
      continue
    fi
    case $mode in
      anchors) flag= ;;
      antianchors) flag="--no-anchors --anti-anchors" ;;
      both) flag=--anti-anchors ;;
      broadened) flag="--anti-anchors --broaden" ;;
      context) flag=--no-anchors ;;
      nocontext) flag="--no-anchors --no-context" ;;
      *) echo "unknown mode $mode"; exit 1 ;;
    esac
    # Broadening starts from the finished "both" rule set.
    if [ $mode = broadened ] && [ ! -f $OUT/pass2-$mode-$size.txt ]; then
      { echo "# broaden"; cat $OUT/rules-both-$size.txt; } > $OUT/pass2-$mode-$size.txt
    fi
    (
      RAYON_NUM_THREADS=1 ./target/release/secondpasstrial --dictionary readlex-phonemic --words $WORDS --rules $size \
        --first-pass-threads $threads --first-pass-scale 2 $flag --max-sweeps 50 --exact --all-stages --pairs \
        --scale 1.0 --adaptive --direct-ties --window 0 --steps-chunk 64 --max-tuple 3 --tuple-threads $threads \
        --checkpoint $OUT/pass2-$mode-$size.txt --output $OUT/rules-$mode-$size.txt >> $OUT/log-$mode-$size.txt 2>&1
      echo "trained $mode $size: $(grep -E "sweep" $OUT/log-$mode-$size.txt | tail -1)"
    ) &
  done
done
wait

# The time for hb-shape to shape the corpus 20 times, best of 5, in ms.
shaping_ms() {
  local best=999999999
  for rep in 1 2 3 4 5; do
    local s=$(date +%s%N)
    hb-shape "$1" --text-file=res/shaping-corpus.txt --num-iterations=20 --no-positions --no-clusters -o /dev/null
    local e=$(( ($(date +%s%N) - s) / 1000000 ))
    if [ $e -lt $best ]; then best=$e; fi
  done
  echo $best
}

error_of() {
  ./target/release/showerrors --rules "$1" --dictionary readlex-phonemic --words $2 --top 0 | sed -n 's/.*total weighted error \([0-9.]*\).*/\1/p'
}

results=$OUT/results.tsv
echo -e "mode\tsize\tlookups\trendering\tmismatches\terror_train\terror_5000\tshaping_ms" > $results
echo -e "base\t0\t0\t-\t0\t-\t-\t$(shaping_ms $BASE_FONT)" >> $results
for st in $SIZES; do
  size=${st%:*}
  for mode in ${MODES//+/ }; do
    rules=$OUT/rules-$mode-$size.txt
    train=$(error_of $rules $WORDS)
    all=$(error_of $rules 5000)
    for rendering in ${RENDERINGS//+/ }; do
      flag=
      if [ $rendering = plain ]; then flag=--plain; fi
      font=$OUT/font-$mode-$size-$rendering.otf
      info=$(scripts/build_font.sh $rules $font --dictionary readlex-phonemic --words 5000 $flag 2>&1 || true)
      lookups=$(echo "$info" | sed -n 's/.* in \([0-9]*\) lookups.*/\1/p' | head -1)
      mismatches=$(echo "$info" | sed -n 's/^\([0-9]*\) of .* shape differently.*/\1/p')
      ms=$(shaping_ms $font)
      echo -e "$mode\t$size\t$lookups\t$rendering\t$mismatches\t$train\t$all\t$ms" | tee -a $results
    done
  done
done
