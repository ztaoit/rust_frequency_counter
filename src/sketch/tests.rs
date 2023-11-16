use crate::sketch::frequency_count_sketch;
use crate::sketch::frequency_count_sketch::FrequencyCountSketch;

#[test]
fn test_bit_count() {
    let i = 23u64;
    println!("{}", frequency_count_sketch::bit_count(i))
}

#[test]
fn test_number_of_leading_zeros() {
    let i = 89;
    println!("{}", frequency_count_sketch::number_of_leading_zeros(i))
}

#[test]
fn test_ceiling_power_of_two() {
    let x = 10;
    println!("{}", frequency_count_sketch::ceiling_power_of_two(x))
}

#[test]
fn test_hash_code() {
    let x = 32;
    let hash_code = frequency_count_sketch::default_hash_code(x);
    println!("{}", hash_code)
}

#[test]
fn test_increment() {
    let mut counter= FrequencyCountSketch::new(20);

    let a = 1;
    counter.increment(a);
    counter.increment(a);
    counter.increment(a);

    println!("{:?}", counter)
}

#[test]
fn test_frequency() {
    let mut counter= FrequencyCountSketch::new(20);

    let a = 1;
    counter.increment(a);
    counter.increment(a);
    counter.increment(a);

    println!("{:?}", counter);

    let f = counter.frequency(a);
    println!("{}", f)
}