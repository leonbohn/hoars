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
