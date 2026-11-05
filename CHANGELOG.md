# Changelog

All notable changes to this project are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the version is 0.x, minor releases may contain breaking changes.

## [Unreleased]

### Fixed

- `first_automaton_split_position` no longer loops forever when an `--ABORT--` occurs before the first `--END--`; aborted automata are skipped and the returned index refers to the end of the first non-aborted automaton in the original input.
