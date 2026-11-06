#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &str| {
    let _ = hoars::parse_hoa_automata(input);
    if let Some(pos) = hoars::first_automaton_split_position(input) {
        assert!(pos <= input.len() && input.is_char_boundary(pos));
    }
});
