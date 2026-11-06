#![no_main]
use hoars::HoaRepresentation;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &str| {
    let _ = HoaRepresentation::try_from(input);
});
