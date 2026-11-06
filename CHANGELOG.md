# Changelog

All notable changes to this project are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the version is 0.x, minor releases may contain breaking changes.

## [Unreleased]

### Added

- `HoaRepresentation::validate`, run by the parser, with the new `FromHoaError::Invalid(ValidationError)`. It rejects a missing `Acceptance:`, repeated `States:`/`AP:`/`acc-name:`/`tool:`/`name:`/`Acceptance:` items, and acceptance sets, states and APs beyond the declared counts. Unsupported versions now give `FromHoaError::UnsupportedVersion`; `v1` and `v1.x` are accepted (the lexer accepts dotted versions such as `v1.1`).
- Implicit edge labels and state labels. A state without label whose edges have none needs exactly `2^|AP|` edges (edge `i` gets the label where AP `j` holds iff bit `j` of `i` is set); a state label is copied onto each of its outgoing edges; mixing labelled and unlabelled edges is a parse error. Implicit and state-labelled automata parse to the same `HoaRepresentation` as their explicit equivalents.
- Alias references (`@name`) in label expressions: `AbstractLabelExpression::Alias(String)`. `HoaRepresentation::unalias` substitutes them recursively in the aliases and in all edge labels and reports undefined or cyclic aliases. Parsing does not unalias; BDD conversion of an unresolved alias returns an error.
- `HeaderItem::Unknown(name, values)` (with `UnknownValue`): unknown header items such as `controllable-AP:` or `spot.state-player:` are now parsed, kept, and written back out instead of being a parse error. Unknown names starting with an uppercase letter log a `tracing` warning. Header names may now contain `.`.

### Changed

- **Breaking:** `Header` is a validated struct instead of a `Vec<HeaderItem>` behind `Deref`/`DerefMut`. Items that may appear once are fields (`version`, `states`, `aps`, `acceptance`, `acceptance_name_info`, `tool`, `name`), repeatable ones are collected (`start`, `aliases`, `properties`, `unknown`), and the accessors are field reads. Build one with `Header::new`, `Header::from_items` or `Header::push`, which reject repeated single items with `ValidationError::DuplicateHeader` (and a missing version with the new `ValidationError::MissingVersion`); `Header::items` lists the items again. The order of the input is not kept: the writer uses a fixed order, unknown items go last, and several `properties:` lines are joined. `Header::from_vec` and the `Vec` methods are gone, `HoaRepresentation::add_header_item` returns a `Result`, `Header::parser` and `HoaRepresentation::parser` yield `Result`s.
- **Breaking:** `Body` derefs to `[State]` only (no `DerefMut`) and stays sorted by state id: `From<Vec<State>>` sorts, `Body::push` inserts at the right place, `Body::state(id)` looks a state up by binary search, `Body::iter_mut` gives mutable states. `HoaRepresentation::from_parts` no longer sorts on its own.

- **Breaking:** state-based acceptance (`State: 0 {1}`) is no longer merged into the outgoing edges. `State` keeps it as `Option<AcceptanceSignature>` (`State::acceptance`, `State::with_acceptance`; `State::from_parts` is unchanged and sets none), `Edge::acceptance_signature` returns only the edge's own sets, and the writer prints the state signature back on the `State:` line. Use the new `Edge::effective_acceptance(&State)` (sorted, deduplicated) to get the merged view; consumers such as the `automata` `hoa` feature must switch to it.

- **Breaking:** `FromHoaError` implements `std::error::Error` (as does `ValidationError`) and its `Display` output has no ANSI colour codes. `LexerError` and `ParserError` now hold `Vec<Diagnostic>` (byte span, message, expected, found) instead of a rendered report; `FromHoaError::render(src, color)` produces the ariadne report. The never-constructed variants `UnsupportedAcceptanceCondition`, `ParseAcceptanceCondition` and `UnsupportedBody` are removed.

- Lexer: strings follow the spec's C-like syntax (`\"` and `\\` are decoded, other escapes are kept verbatim), comments nest, and `--ABORT--` lexes as `Token::Abort` (it used to be `Token::BodyEnd`). `Token::Bool` is gone; `t`/`f` are identifiers.
- `--ABORT--` handling works on tokens instead of substrings: `from_hoa` only rejects real aborts (not one inside a string, nor `foo--ABORT--`), and `parse_hoa_automata` and `first_automaton_split_position` lex the input once and split the token stream, so an automaton following an aborted one in the same chunk is returned. A lexer error now makes `parse_hoa_automata` return nothing, and `first_automaton_split_position` return `None`.
- `Property` and `AcceptanceName` gained an `Other(String)` variant, and their `TryFrom<String>` impls became infallible `From<String>`. Unknown `properties:` values (e.g. `semi-deterministic`) and `acc-name:` names (e.g. `Rabin-like`) now parse and are written back verbatim.
- Accessors no longer `debug_assert` header multiplicities that the HOA spec does not require, so they work on valid files without `States:`, `Start:`, `acc-name:` or `AP:`:
  - `HoaRepresentation::aps` returns `&[String]` (was `&Vec<String>`) and is empty when the header has no `AP:`;
  - `num_aps` returns `u16` (was `u8`, which panicked above 255 APs);
  - `acceptance` returns `Option<HoaAcceptance>`;
  - `Header::acceptance_name` returns `Option<AcceptanceName>`;
  - `num_states`, `start` and `HoaRepresentation::acceptance_name` keep their types but no longer assert.
- `AbstractLabelExpression::try_into_bdd` now takes a pre-built `&BddVariableSet` (and `&self`), so a whole automaton can be converted with one variable set; `try_into_prop` likewise takes `&self`, and both it and `try_into_hoa_expression` take the AP count as `u16`. AP `i` maps to `BddVariable::from_index(i)`, so the `vars` slice is no longer needed.
- Removed the unused `build_vars` helper.

### Fixed

- Integers that do not fit the target type (e.g. `States: 99999999999`) are reported as parse errors instead of panicking, and an AP identifier that does not fit a `u16` is no longer silently truncated.
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
