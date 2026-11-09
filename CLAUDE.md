# CLAUDE.md

`hoars` parses (and, partially, writes) the HOA (Hanoi Omega Automata) format, <https://adl.github.io/hoaf/>. It is the parser behind the `hoa` feature of the `automata` crate (`leonbohn/automata`). Edition 2024, version 0.4.0, MIT.

## History

This repository's history is the original standalone `hoars` history (to the 0.2 release in April 2024) followed by the commits that were made to `crates/hoars` while the code lived in the `leonbohn/frieda` monorepo (non-merge commits, replayed; manifest conflicts resolved in favor of the monorepo version). A final commit makes the crate standalone again.

## Commands

```shell
cargo build --all-targets
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
./scripts/check
```

## Layout

- `src/lexer.rs`: tokens (`chumsky`-based). `src/input.rs`: parsing entry points, `ariadne` diagnostics.
- `src/header.rs`, `src/body.rs`, `src/value.rs`: parsers/types for the header items, the body (states, edges), and values (integers, acceptance expressions, labels as `AbstractLabelExpression`; there is no BDD library, interpreting a label is up to the user).
- `src/format.rs`, `src/output.rs`: writing HOA. `src/interpretation.rs`: unaliasing and interpretation of the parsed automaton.
- `src/lib.rs`: `HoaAutomaton` (header + body), `from_parts`, `from_hoa` helpers.
- `hoa/`: sample HOA files used by tests (`spec/`, `spot/`; `bench/` is for benchmarks only); `doc/hoaf.pdf`: the format specification.

## Known sharp edges

- MSRV is 1.88 (`rust-version`), checked in CI for the library only.
- Parsers use `chumsky` 0.9 (`Simple<Token>` errors), flagged with `allow(clippy::result_large_err)` in several places.
- 52 unit tests, including one that parses, round-trips and converts every fixture in `hoa/` (`hoa/spec/` holds the spec examples, `hoa/spot/` real Spot output); no fuzzing.
- Version `0.4.0` is the first release since crates.io `0.2.0` (0.3.0 was never published); breaking changes include the reworked `Header`/`Body`/error API.

## Benchmarks

Same approach as `automata` (see its `docs/benchmarking.md`): instruction counts under valgrind
with gungraun (needs `valgrind` and `gungraun-runner` 0.20.0), deterministic, differences above
~1% are real. Inputs are the generator of REFACTOR.md P2 (`benches/common`) and `hoa/bench/`.

```shell
cargo bench --bench parse               # also: verify, write (~80 s each)
cargo bench --bench parse -- --save-baseline=main   # on main, then on the branch:
cargo bench --bench parse -- --baseline=main        # prints the diff
HOARS_BENCH_XL=1 cargo bench --bench wallclock      # criterion, includes the 200k-state input
```

Put the before/after table in the PR. For the effect on `automata`, run its `automata-bench compare`
with `[patch.crates-io] hoars = { path = "../hoars" }` applied on both sides.
