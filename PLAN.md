# PLAN: hoars

Goal: a small, robust, well-tested HOA v1 parser/printer crate that `automata` depends on from crates.io.

- [ ] Push this repository and release `0.2.1` (or `0.3.0` if the API changes) to crates.io so `automata` can drop its git dependency.
- [ ] Add `rust-version` (MSRV) and a CI matrix (Linux/macOS/Windows, MSRV, `--all-features`); CI currently only builds on Windows and macOS.
- [ ] Complete HOA v1 coverage (all header items and acceptance names, aliases, `--ABORT--`), and conformance tests against the spec examples and Spot's output.
- [ ] Round-trip tests (parse, print, parse) and fuzzing of the lexer/parser (`cargo-fuzz`).
- [ ] Replace `lazy_static` with `std::sync::LazyLock`; revisit `chumsky` 1.x and error types; document the public API (`#![deny(missing_docs)]`).
- [ ] README: usage example, supported subset of the format, changelog (the 0.2.1 entry is in the README).
