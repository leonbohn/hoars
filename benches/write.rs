//! `to_hoa` on generated rings and fixtures (parsing is setup, not measured).

mod common;

use gungraun::prelude::*;
use hoars::{HoaRepresentation, output::to_hoa};
use std::hint::black_box;

fn generated(n: usize) -> HoaRepresentation {
    common::parse(&common::generated(n))
}
fn parsed(texts: Vec<String>) -> Vec<HoaRepresentation> {
    texts.iter().map(|t| common::parse(t)).collect()
}
fn ltl() -> Vec<HoaRepresentation> {
    parsed(common::ltl())
}
fn synthetic() -> Vec<HoaRepresentation> {
    parsed(common::synthetic())
}
fn spot() -> Vec<HoaRepresentation> {
    parsed(common::spot())
}

#[library_benchmark]
#[benches::generated(args = [250, 1_000, 4_000], setup = generated)]
fn write_generated(aut: HoaRepresentation) -> usize {
    black_box(to_hoa(&aut)).len()
}

#[library_benchmark]
#[bench::ltl(setup = ltl)]
#[bench::synthetic(setup = synthetic)]
#[bench::spot(setup = spot)]
fn write_fixtures(auts: Vec<HoaRepresentation>) -> usize {
    auts.iter().map(|a| black_box(to_hoa(a)).len()).sum()
}

library_benchmark_group!(name = write; benchmarks = write_generated, write_fixtures);
main!(library_benchmark_groups = write);
