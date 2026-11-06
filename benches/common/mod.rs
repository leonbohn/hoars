//! Deterministic inputs shared by all benchmarks. No randomness: inputs are pure functions of
//! their size or committed files.
#![allow(dead_code)]

use hoars::HoaRepresentation;
use std::{fmt::Write, path::Path};

/// The input generator from REFACTOR.md (P2): a ring-like automaton with `n` states, 2 APs and
/// 4 edges per state.
pub fn generated(n: usize) -> String {
    let mut s = String::with_capacity(n * 80);
    s.push_str("HOA: v1\n");
    writeln!(s, "States: {n}").unwrap();
    s.push_str("Start: 0\nAP: 2 \"a\" \"b\"\nAcceptance: 1 Inf(0)\n--BODY--\n");
    for i in 0..n {
        writeln!(s, "State: {i}").unwrap();
        writeln!(s, "[0 & 1] {} {{0}}", (i + 1) % n).unwrap();
        writeln!(s, "[!0 & 1] {}", (i + 7) % n).unwrap();
        writeln!(s, "[0 & !1] {}", (i * 3) % n).unwrap();
        writeln!(s, "[!0 & !1] {i}").unwrap();
    }
    s.push_str("--END--\n");
    s
}

pub fn parse(text: &str) -> HoaRepresentation {
    HoaRepresentation::try_from(text).expect("benchmark input parses")
}

/// Texts of all `.hoa` files in `hoa/bench/<dirs>`, sorted by path (Spot-generated, see
/// `hoa/bench/README.md`).
pub fn fixture_texts(dirs: &[&str]) -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("hoa/bench");
    let mut files: Vec<_> = dirs
        .iter()
        .flat_map(|d| std::fs::read_dir(root.join(d)).expect("fixture directory"))
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "hoa"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|p| std::fs::read_to_string(p).expect("fixture"))
        .collect()
}

pub fn ltl() -> Vec<String> {
    fixture_texts(&["ltl"])
}
pub fn synthetic() -> Vec<String> {
    fixture_texts(&["random", "hard"])
}
pub fn spot() -> Vec<String> {
    fixture_texts(&["spot"])
}
