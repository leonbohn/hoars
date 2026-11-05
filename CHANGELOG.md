# Changelog

All notable changes to this project are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the version is 0.x, minor releases may contain breaking changes.

## [Unreleased]

### Changed

- `AbstractLabelExpression::try_into_bdd` now takes a pre-built `&BddVariableSet` (and `&self`), so a whole automaton can be converted with one variable set; `try_into_prop` likewise takes `&self`. AP `i` maps to `BddVariable::from_index(i)`, so the `vars` slice is no longer needed.
- Removed the unused `build_vars` helper.

### Fixed

- The writer (`output::to_hoa`) now produces HOA that parses back to the same automaton:
  - labels print as `[<expr>]` with precedence-aware parentheses (a disjunction inside a conjunction, a non-atom inside a negation), instead of the Rust `Debug` representation;
  - `LabelExpression::Expression(Bdd)` is printed as a DNF built from the BDD's satisfying clauses;
  - `properties: unambiguous` is spelled correctly (was `unabmiguous`);
  - `tool:` and `name:` values are quoted, and `tool:` no longer prints a trailing space when there is no version;
  - all strings (AP names, `tool:`/`name:` values, state comments) escape `\` and `"`;
  - `State` no longer ends with a newline, so states are no longer separated by blank lines, and edges omit the trailing space when they have no acceptance signature.
- Label expressions are converted to BDDs by a recursive fold, so nested conjunctions and disjunctions (e.g. `[0&!1 | !0&1]` or `[(0|1) & !1]`) are no longer rejected.
- `[0 & !0]` now converts to `false` instead of an incorrect BDD.
- An out-of-bounds AP identifier in a nested expression (e.g. `[0 & !7]` with 2 APs) returns an error instead of panicking.
- `first_automaton_split_position` no longer loops forever when an `--ABORT--` occurs before the first `--END--`; aborted automata are skipped and the returned index refers to the end of the first non-aborted automaton in the original input.
