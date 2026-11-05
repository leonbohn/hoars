//! `verify()` and `validate()` on generated rings (parsing is setup, not measured).

mod common;

use gungraun::prelude::*;
use hoars::HoaRepresentation;
use std::hint::black_box;

fn setup(n: usize) -> HoaRepresentation {
    common::parse(&common::generated(n))
}

#[library_benchmark]
#[benches::generated(args = [250, 1_000, 4_000], setup = setup)]
fn verify(aut: HoaRepresentation) -> bool {
    black_box(aut.verify()).is_ok()
}

#[library_benchmark]
#[benches::generated(args = [250, 1_000, 4_000], setup = setup)]
fn validate(aut: HoaRepresentation) -> bool {
    black_box(aut.validate()).is_ok()
}

library_benchmark_group!(name = verify_group; benchmarks = verify, validate);
main!(library_benchmark_groups = verify_group);
