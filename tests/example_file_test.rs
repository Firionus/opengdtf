use std::collections::HashMap;

use example_files::{
    check_for_duplicate_filenames, opened_examples_iter, parse_expected_toml, parsed_examples_iter,
    OutputEnum,
};
use opengdtf::hash::hash_gdtf_to_string;
use pretty_assertions::assert_eq;

#[test]
fn expected_toml_has_no_duplicate_filenames() {
    let expected = parse_expected_toml();
    check_for_duplicate_filenames(expected);
}

#[test]
fn fixtures_from_expected_toml_are_in_examples() {
    let expected = parse_expected_toml();
    let mut hashes_in_examples = HashMap::<String, String>::new();
    for (entry, file) in opened_examples_iter() {
        let key = hash_gdtf_to_string(file).unwrap();
        let filename = entry.file_name().to_str().unwrap().to_string();
        match hashes_in_examples.get(&key) {
            Some(collision_filename) => panic!(
                "hash collision between '{}' and '{}'; GDTF files likely have the same content and one of them should be removed",
                collision_filename, filename
            ),
            None => hashes_in_examples.insert(key, filename.clone()),
        };
    }
    let mut missing = Vec::<String>::new();
    for (expected_key, expected_entry) in expected {
        if hashes_in_examples.get(&expected_key).is_none() {
            missing.push(format!(
                "'{}' with hash {}",
                expected_entry.filename, expected_key
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "fixtures from 'expected.toml' are missing in examples:
{missing:#?}
please add these fixtures to the examples folder, e.g. by downloading them from gdtf-share.com. Alternatively, delete these entries in 'expected.toml'"
    );
}

#[test]
fn fixtures_from_examples_are_in_expected_toml() {
    let expected = parse_expected_toml();
    let mut missing = Vec::<String>::new();
    for (entry, file) in opened_examples_iter() {
        let key = hash_gdtf_to_string(file).unwrap();
        if expected.get(&key).is_none() {
            missing.push(entry.file_name().to_str().unwrap().to_string())
        }
    }
    assert!(
        missing.is_empty(),
        "fixtures from examples are missing in 'expected.toml':
{missing:#?}
please add these fixtures to 'expected.toml' by running `cargo run --bin update_expected` and check the diff of 'expected.toml'"
    );
}

#[test]
fn expected_toml_matches_examples_output() {
    let expected = parse_expected_toml();
    for (_entry, file, parsed_result) in parsed_examples_iter() {
        let key = hash_gdtf_to_string(file).unwrap();
        let expected_output = match expected.get(&key) {
            Some(v) => &v.output_enum,
            None => continue,
        };
        let parsed_entry = OutputEnum::from(parsed_result);

        assert_eq!(expected_output, &parsed_entry);
    }
}
