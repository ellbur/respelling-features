#!/bin/bash
# How well the first pass and the pass-2 tuple sweeps scale with threads.
set -e
T=./target/release/secondpasstrial
N=$(nproc)
for threads in 1 4 8 16 $N; do
  rm -f working/first-pass-300-20.txt
  echo "--- first pass, 300 words, 20 rules, $threads threads"
  RAYON_NUM_THREADS=$threads $T --words 300 --rules 20 --max-sweeps 0 | grep "First pass took"
done
for threads in 1 4 8 16 $N; do
  echo "--- pass 2 with triples, tuple threads $threads"
  RAYON_NUM_THREADS=1 $T --words 300 --rules 30 --max-sweeps 30 --exact --all-stages --pairs --scale 1.0 --adaptive --direct-ties --window 0 --steps-chunk 64 --max-tuple 3 --tuple-threads $threads \
    | grep "tuple sweep" | awk '{gsub("s","",$NF); t+=$NF; last=$6} END {printf "tuple sweeps %.1fs, final score %s\n", t, last}'
done
