#![no_main]
use hoars::HoaRepresentation;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &str| {
    if let Ok(aut) = HoaRepresentation::try_from(input) {
        let printed = hoars::output::to_hoa(&aut);
        let reparsed = HoaRepresentation::try_from(printed.as_str())
            .unwrap_or_else(|e| panic!("reprint does not parse: {e}\n{printed}"));
        assert_eq!(reparsed, aut, "round trip changed the automaton:\n{printed}");
    }
});
