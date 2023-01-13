use std::fs::File;
use std::io::Write;

use chrono::Utc;
use example_files::{
    check_for_duplicate_filenames, parse_expected_toml, parsed_examples_iter, ExpectedEntry,
    EXPECTED_TOML_PATH,
};
use opengdtf::hash::gdtf_hash_string;

fn main() {
    let mut expected = parse_expected_toml();

    println!("iterating over example files");

    for (entry, file, parsed_result) in parsed_examples_iter() {
        println!("{entry:?}");

        let key = gdtf_hash_string(file).unwrap();

        let output_enum = parsed_result.into();

        let comment = if let Some(existing_entry) = expected.get(&key) {
            if existing_entry.output_enum == output_enum {
                continue;
            }
            existing_entry.comment.clone()
        } else {
            "".to_string()
        };

        expected.insert(
            key,
            ExpectedEntry {
                filename: format!("{}", entry.file_name().to_string_lossy()),
                saved_on: Utc::now(),
                comment,
                output_enum,
            },
        );
    }

    let serialized = toml::to_string_pretty(&expected).unwrap();
    let mut output_file = File::create(&*EXPECTED_TOML_PATH).unwrap();
    write!(output_file, "{}", &serialized).unwrap();

    check_for_duplicate_filenames(expected);
}
