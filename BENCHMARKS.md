# Benchmark baselines

Recorded before any P2 change, at commit `0a24d83` (+ benches). Commands: see CLAUDE.md. Instruction
counts are deterministic (valgrind/gungraun); wall-clock numbers are from one machine
(AMD Ryzen 5 3600 6-Core Processor, 12 threads, release profile) and only indicative.

## Instructions (gungraun)

| Benchmark | Instructions |
|---|---:|
| parse_generated 250 | 103,388,130 |
| parse_generated 1000 | 412,824,152 |
| parse_generated 4000 | 1,654,866,754 |
| parse_fixtures ltl | 248,160,210 |
| parse_fixtures synthetic | 329,139,585 |
| parse_fixtures spot | 1,205,177,431 |
| verify 250 | 843,353 |
| verify 1000 | 3,766,101 |
| verify 4000 | 21,334,543 |
| validate 250 | 941,618 |
| validate 1000 | 3,765,422 |
| validate 4000 | 14,964,359 |
| write_generated 250 | 3,778,750 |
| write_generated 1000 | 15,490,652 |
| write_generated 4000 | 61,744,611 |
| write_fixtures ltl | 7,588,526 |
| write_fixtures synthetic | 11,754,868 |
| write_fixtures spot | 38,898,446 |

## Wall clock (criterion, median of 10 samples; `HOARS_BENCH_XL=1` for 200k)

| Operation | N = 1,000 | N = 10,000 | N = 200,000 (15 MB) |
|---|---:|---:|---:|
| parse | 48.4 ms (1.31 MiB/s) | 501 ms (1.36 MiB/s) | **10.63 s** (1.42 MiB/s) |
| verify | 36.7 µs | 2.77 ms | **1.141 s** |
| write | 1.13 ms | 11.96 ms | 240 ms |

These match the numbers in REFACTOR.md P2 (9.9 s, 1.18 s).

## Issue 11, step 1: chumsky 0.9 → 0.13 (same two-stage lexer/parser, tokens borrow from the input)

| Benchmark | Instructions | vs. baseline |
|---|---:|---:|
| parse_generated 250 | 42,185,713 | −59.2 % (2.45×) |
| parse_generated 1000 | 169,356,712 | −59.0 % (2.44×) |
| parse_generated 4000 | 675,566,420 | −59.2 % (2.45×) |
| parse_fixtures ltl | 103,141,813 | −58.4 % |
| parse_fixtures synthetic | 137,022,599 | −58.4 % |
| parse_fixtures spot | 497,143,446 | −58.7 % |

Wall clock parse: 1k 16.97 ms (3.74 MiB/s), 10k 171.6 ms (3.97 MiB/s), **200k (15 MB) 3.82 s** (3.95 MiB/s)
against 10.63 s before (2.8×). The ≥ 10× target of issue 11 is not reached by this step alone.

## Issue 11, step 2: cheaper lexer (profile-driven)

Callgrind on a 2000-state input (336M instructions after step 1) showed lexing at 78 %, parsing at 19 %.
About 22 % of the total was chumsky's tracking of expected tokens (`add_alt`, `RichPattern::slice_contains`,
allocations) for failed alternatives in the lexer, even though no error ever happens. Two changes:

1. Single-character tokens (`op`, `paren`) are tried first in the `token` choice (336M → 249M).
2. The lexer uses `Cheap<Span>` errors instead of `Rich<char>`; `input::lex` rebuilds a message that names the
   offending character (249M → 127M, lexing 263M → 54M).

Trying to avoid the `Vec` per label conjunction/disjunction gained < 2 % and was dropped.

| Benchmark | Instructions | vs. baseline |
|---|---:|---:|
| parse_generated 250 | 15,503,980 | −85.0 % (6.7×) |
| parse_generated 1000 | 62,334,367 | −84.9 % (6.6×) |
| parse_generated 4000 | 245,841,778 | −85.1 % (6.7×) |
| parse_fixtures ltl | 34,725,034 | −86.0 % (7.1×) |
| parse_fixtures synthetic | 47,721,647 | −85.5 % (6.9×) |
| parse_fixtures spot | 170,116,507 | −85.9 % (7.1×) |

Wall clock parse: 1k 5.9 ms (10.7 MiB/s), 10k 68 ms (10.0 MiB/s), **200k (15 MB) 1.77 s** (8.5 MiB/s), 6.0× faster
than the 10.63 s baseline. Remaining: parsing is now about half of the time (the token `Vec`, `Rich` tracking in
the token parsers, per-edge allocations).

## Issue 11, step 3: cheap errors first, `choice` for the token alternatives

- All token parsers are generic over the error type (`HoaErr`). `from_hoa` parses with `EmptyErr` first and
  only re-parses with `Rich` if that fails, so diagnostics are unchanged. (Parse stage on the 2000-state probe:
  64M → 47M instructions.)
- The lexer's `.or` chain became `choice((...))`: no measurable effect (+1 %, within what reordering costs), kept for
  readability as chumsky's docs recommend.
- `EmptyErr` instead of `Cheap` in the lexer would save another ~4 % but needs a second lexing pass for the error
  position; not done.

| Benchmark | Instructions | vs. baseline |
|---|---:|---:|
| parse_generated 250 | 13,464,754 | −87.0 % (7.7×) |
| parse_generated 1000 | 54,215,079 | −86.9 % (7.6×) |
| parse_generated 4000 | 213,403,011 | −87.1 % (7.8×) |
| parse_fixtures ltl | 31,136,007 | −87.5 % (8.0×) |
| parse_fixtures synthetic | 42,292,980 | −87.2 % (7.8×) |
| parse_fixtures spot | 150,453,265 | −87.5 % (8.0×) |

Wall clock parse: 1k 5.1 ms (12.4 MiB/s), 10k 54.9 ms, **200k (15 MB) 1.51 s** (10.0 MiB/s), 7.1× faster than 10.63 s.

## Issue 11, step 4: lexing and parsing fused

The grammar now runs directly on `&str`: each token parser consumes the layout (whitespace, nested comments;
a small `custom` parser) behind it, so there is no token `Vec` and no second pass. `lexer.rs` is only used to
split files with several automata and to diagnose failures (lexical error, `--ABORT--`) after a failed parse.
The fast attempt still uses `EmptyErr`; `Rich<char>` is only used for the re-parse that builds the report.

| Benchmark | Instructions | vs. baseline |
|---|---:|---:|
| parse_generated 250 | 4,513,809 | −95.6 % (22.9×) |
| parse_generated 1000 | 19,042,427 | −95.4 % (21.7×) |
| parse_generated 4000 | 72,730,762 | −95.6 % (22.8×) |
| parse_fixtures ltl | 9,225,060 | −96.3 % (26.9×) |
| parse_fixtures synthetic | 13,364,209 | −95.9 % (24.6×) |
| parse_fixtures spot | 49,883,774 | −95.9 % (24.2×) |

Wall clock parse: 1k 1.83 ms (34.6 MiB/s), 10k 20.6 ms, **200k (15 MB) 0.547 s** (27.6 MiB/s), **19.4× faster**
than the 10.63 s baseline: the ≥ 10× goal of issue 11 is met.

## Issue 12: `verify()` without `Vec::contains`

Ids are collected, sorted (`sort_unstable`) and checked for equal neighbours; the body is not assumed to be sorted.
`validate()` already checked the ranges of start states and edge targets and is unchanged.

Wall clock `verify`: 1k 36.7 µs → 1.1 µs, 10k 2.77 ms → 11.6 µs, **200k 1.141 s → 0.4 ms** (goal: < 50 ms).
Instruction counts of `verify` (250 / 1000 / 4000): 0.57M / 3.19M / 9.20M (before 0.84M / 3.77M / 21.3M). They include
dropping the automaton that the benchmark function takes by value, which now dominates and is why they do not shrink
as much as the time; the `validate` numbers moved (−24 % to ±0) with the parser's allocation pattern, its code is unchanged.

## Issue 13: small fixes

`aliases()` keeps its signature but is built on the new borrowing `alias_iter()`; `unalias` uses the latter.
`Header::get_version` returns `Option<&str>` (breaking). State and transition acceptance are merged with one
exactly-sized allocation. The per-parse `Instant::now()` / `tracing::info!` calls are gone. (`aps()` already returned a
slice.) Instruction counts do not move for parse/verify (±0.1 %, noise); `write` is 1.4–6.4 % lower than the
baseline above (first `write` run since the parser changes; the writer itself was not touched).

## Final state (all of P2), against the baseline at the top

| | Baseline | Now |
|---|---:|---:|
| parse, 200k states / 15 MB | 10.63 s | 0.547 s (19.4×) |
| verify, 200k states | 1.141 s | 0.4 ms |
| write, 200k states | 240 ms | not re-measured |
| parse_generated 1000 (instructions) | 412,824,152 | 19,056,307 (21.7×) |
| parse_fixtures spot | 1,205,177,431 | 49,909,429 (24.1×) |
