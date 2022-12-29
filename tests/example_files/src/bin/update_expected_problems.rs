use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

use chrono::Utc;
use example_files::{
    examples_update_output_iter, hash::hash_gdtf, parse_expected_problems, ErrorInfo,
    ExpectedEntry, OutputEnum, ProblemInfo, EXPECTED_PROBLEMS_PATH,
};

fn main() {
    let mut expected_problems = parse_expected_problems();

    println!("iterating over example files");
    for (entry, file, output) in examples_update_output_iter() {
        println!("{entry:?}");

        let key = hash_gdtf(file);

        let output_enum = match output {
            Ok(parsed) => OutputEnum::Ok(ProblemInfo {
                manufacturer: parsed.gdtf.manufacturer,
                name: parsed.gdtf.name,
                fixture_type_id: parsed.gdtf.fixture_type_id.to_string(),
                problems: parsed.problems.into_iter().map(|p| p.to_string()).collect(),
            }),
            Err(e) => OutputEnum::Err(ErrorInfo {
                error: e.to_string(),
            }),
        };

        let comment = if let Some(existing_entry) = expected_problems.get(&key) {
            if existing_entry.output_enum == output_enum {
                continue;
            }
            existing_entry.comment.clone()
        } else {
            "".to_string()
        };

        expected_problems.insert(
            key,
            ExpectedEntry {
                filename: format!("{}", entry.file_name().to_string_lossy()),
                saved_on: Utc::now(),
                comment,
                output_enum,
            },
        );
    }
    let serialized = toml::to_string_pretty(&expected_problems).unwrap();
    let mut output_file = File::create(&*EXPECTED_PROBLEMS_PATH).unwrap();
    write!(output_file, "{}", &serialized).unwrap();

    // check for duplicate original filenames
    let mut filename_set = HashMap::<&String, u32>::new();
    for original_filename in expected_problems.values().map(|v| &v.filename) {
        if !filename_set.contains_key(original_filename) {
            filename_set.insert(original_filename, 1);
        } else {
            let prev_count = filename_set[original_filename];
            filename_set.insert(original_filename, prev_count + 1);
        }
    }
    let duplicates: HashMap<&String, u32> = filename_set
        .into_iter()
        .filter(|(_, c)| c != &1u32)
        .collect();
    if !duplicates.is_empty() {
        println!("\nWARNING entries with duplicate filenames:");
        println!("{:#?}", duplicates);
        println!("This probably means a GDTF file was modified without changing its filename.");
        println!("The stale entries in `expected_problems.toml` should be removed.");
    }
}
