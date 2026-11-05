//! Parsing: `HoaRepresentation::try_from` on generated rings of growing size (the slope shows
//! linear vs. superlinear behaviour) and on the Spot fixtures in `hoa/bench`.

mod common;

use gungraun::prelude::*;
use std::hint::black_box;

fn parse_all(texts: Vec<String>) -> usize {
    texts
        .iter()
        .map(|t| black_box(common::parse(t)).body().into_iter().count())
        .sum()
}

#[library_benchmark]
#[benches::generated(args = [250, 1_000, 4_000], setup = common::generated)]
fn parse_generated(text: String) -> usize {
    black_box(common::parse(&text)).body().into_iter().count()
}

#[library_benchmark]
#[bench::ltl(setup = common::ltl)]
#[bench::synthetic(setup = common::synthetic)]
#[bench::spot(setup = common::spot)]
fn parse_fixtures(texts: Vec<String>) -> usize {
    parse_all(texts)
}

library_benchmark_group!(name = parse; benchmarks = parse_generated, parse_fixtures);
main!(library_benchmark_groups = parse);
