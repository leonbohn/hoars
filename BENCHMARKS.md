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
