use std::fs::File;
use std::time::Duration;
use criterion::Criterion;
use transaction_processor::input::Input;
use transaction_processor::transaction::drill;

fn compare_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("small-inputs");
    group.bench_function("1-client-20-deposits",
                         |b| b.iter(|| drill(Input::from(File::open("benches/1-client-20-deposits.in").unwrap()), true, Some(Duration::from_millis(100)), false)));
    group.bench_function("20-clients-20-deposits",
                         |b| b.iter(|| drill(Input::from(File::open("benches/20-clients-20-deposits.in").unwrap()), true, Some(Duration::from_millis(100)), false)));
    group.finish();
}

fn compare_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("large-inputs");
    group.bench_function("1-client-100-deposits",
                         |b| b.iter(|| drill(Input::from(File::open("benches/1-client-100-deposits.in").unwrap()), true, Some(Duration::from_millis(100)), false)));
    group.bench_function("50-clients-100-deposits",
                         |b| b.iter(|| drill(Input::from(File::open("benches/50-clients-100-deposits.in").unwrap()), true, Some(Duration::from_millis(100)), false)));
    group.bench_function("100-clients-100-deposits",
                         |b| b.iter(|| drill(Input::from(File::open("benches/100-clients-100-deposits.in").unwrap()), true, Some(Duration::from_millis(100)), false)));

    group.finish();
}

fn main() {
    let mut c = Criterion::default();
    compare_small(&mut c);
    compare_large(&mut c);
}
