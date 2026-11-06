#!/usr/bin/env bash
# Regenerates hoa/bench/spot (needs Spot: genltl, ltl2tgba, randaut). The DPA fixtures in
# hoa/bench/{ltl,random,hard} come from leonbohn/automata (benches/fixtures). Regenerating
# changes the benchmark inputs and therefore invalidates earlier benchmark numbers.
set -euo pipefail
OUT=${1:-hoa/bench/spot}
rm -rf "$OUT"; mkdir -p "$OUT"
genltl --gf-equiv=2 | ltl2tgba -B --hoaf=t > "$OUT/buchi-gf-equiv-2.hoa"
genltl --gh-q=2 | ltl2tgba -D -G --hoaf=t > "$OUT/gen-gh-q-2.hoa"
genltl --ms-phi-s=3 | ltl2tgba -B -S > "$OUT/buchi-state-ms-3.hoa"
randaut -n1 -Q30 -e0.3 --seed=1 -A'Rabin 3' 2 > "$OUT/rabin-nondet-n30.hoa"
randaut -n1 -Q30 --seed=2 -A'Streett 2' 3 > "$OUT/streett-nondet-n30.hoa"
randaut -n1 -Q120 -e0.25 --seed=3 -A'Rabin 3' 3 > "$OUT/rabin-nondet-n120-a3.hoa"
randaut -n1 -Q120 -e0.25 --seed=4 -A'Streett 3' 3 > "$OUT/streett-nondet-n120-a3.hoa"
randaut -n1 -Q60 --seed=5 -A'generalized-Buchi 3' 2 > "$OUT/genbuchi-n60-a2.hoa"
