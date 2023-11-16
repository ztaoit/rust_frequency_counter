use count_min_sketch::CountMinSketch64;
use criterion::{Criterion, criterion_group, criterion_main};
use rust_frequency_counter::sketch::frequency_count_sketch::FrequencyCountSketch;

fn test_count_min_sketch(max: usize) {
    let mut cms = CountMinSketch64::<u64>::new(max , 0.99, 2.0).unwrap();
    for i in 0..max {
        cms.increment(&(i as u64));
    }
}

fn test_frequency_count_sketch(max: usize) {
    let mut sketch = FrequencyCountSketch::new(max);
    for i in 0..max {
        sketch.increment(i)
    }
}

fn sketch_1_benchmark(c: &mut Criterion) {
    c.bench_function("sketch1", |b| b.iter(|| test_count_min_sketch(100000)));
}

fn sketch_2_benchmark(c: &mut Criterion) {
    c.bench_function("sketch2", |b| b.iter(|| test_frequency_count_sketch(100000)));
}

criterion_group!(benches, sketch_1_benchmark, sketch_2_benchmark);
criterion_main!(benches);