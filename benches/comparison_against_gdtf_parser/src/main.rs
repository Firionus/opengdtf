use std::time::Instant;

use example_files::{examples_iter, parsed_examples_iter};

fn main() {
    let fixtures = examples_iter().count();
    println!("parsing {fixtures} example files with both crates and measuring time...");

    let mut results = 0;
    let start = Instant::now();
    for entry in examples_iter() {
        if gdtf_parser::Gdtf::try_from(entry.path()).is_ok() {
            results += 1
        }
    }
    let stop = Instant::now();
    let gdtf_parser_time = stop.duration_since(start);
    println!("gdtf-parser successfully parsed {results} fixtures");

    let mut results = 0;
    let start = Instant::now();
    for (_entry, _file, parsed) in parsed_examples_iter() {
        if parsed.is_ok() {
            results += 1
        }
    }
    let stop = Instant::now();
    let opengdtf_time = stop.duration_since(start);
    println!("gdtf-parser successfully parsed {results} fixtures");

    println!("gdtf-parser took {} s", gdtf_parser_time.as_secs_f32());
    println!("opengdtf took {} s", opengdtf_time.as_secs_f32());
    println!(
        "opengdtf relative time to gdtf-parser: {}",
        opengdtf_time.as_nanos() as f64 / gdtf_parser_time.as_nanos() as f64
    )
}
