//! Wall-clock times (criterion) for real magnitudes; never gated. The 200k-state input of
//! REFACTOR.md (P2) only runs with `HOARS_BENCH_XL=1`: parsing it takes several seconds.

mod common;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

fn wallclock(c: &mut Criterion) {
    let mut sizes = vec![1_000, 10_000];
    if std::env::var_os("HOARS_BENCH_XL").is_some() {
        sizes.push(200_000);
    }
    let mut g = c.benchmark_group("wallclock");
    g.sample_size(10);
    for n in sizes {
        let text = common::generated(n);
        g.throughput(Throughput::Bytes(text.len() as u64));
        g.bench_function(format!("parse/{n}"), |b| {
            b.iter(|| black_box(common::parse(&text)))
        });
        let aut = common::parse(&text);
        g.bench_function(format!("verify/{n}"), |b| {
            b.iter(|| black_box(aut.verify()))
        });
        g.bench_function(format!("write/{n}"), |b| {
            b.iter(|| black_box(hoars::output::to_hoa(&aut)))
        });
    }
    g.finish();
}

criterion_group!(benches, wallclock);
criterion_main!(benches);
