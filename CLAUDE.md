# CLAUDE.md

`hoars` parses (and, partially, writes) the HOA (Hanoi Omega Automata) format, <https://adl.github.io/hoaf/>. It is the parser behind the `hoa` feature of the `automata` crate (`leonbohn/automata`). Edition 2024, version 0.2.1, MIT.

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
- `src/header.rs`, `src/body.rs`, `src/value.rs`: parsers/types for the header items, the body (states, edges), and values (integers, acceptance expressions, labels via `biodivine-lib-bdd`).
- `src/format.rs`, `src/output.rs`: writing HOA. `src/interpretation.rs`: unaliasing and interpretation of the parsed automaton.
- `src/lib.rs`: `HoaAutomaton` (header + body), `from_parts`, `from_hoa` helpers.
- `hoa/`: sample HOA files used by tests; `doc/hoaf.pdf`: the format specification.

## Known sharp edges

- Parsers use `chumsky` 0.9 (`Simple<Token>` errors), flagged with `allow(clippy::result_large_err)` in several places.
- Test coverage is small (10 unit tests); no fuzzing.
- Not yet published beyond crates.io `0.2.0`; `0.2.1` exists only here.
