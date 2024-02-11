use std::{
    collections::HashMap,
    error::Error,
    io::{BufReader, Cursor},
};

use example_files::{
    check_for_duplicate_filenames, opened_examples_iter, parse_expected_toml, parsed_examples_iter,
    OutputEnum,
};
use opengdtf::{
    hash::hash_gdtf_to_string,
    parse::{self, parse_low_level_gdtf, ParsedGdtf},
    parse_gdtf, serialize_gdtf, serialize_low_level_gdtf, ValidatedGdtf,
};
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
                "hash collision between '{collision_filename}' and '{filename}'; GDTF files likely have the same content and one of them should be removed"
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

#[test]
fn examples_roundtrip_deser_ser_deser() -> Result<(), Box<dyn Error>> {
    for (_entry, _file, parsed_result) in parsed_examples_iter() {
        let mut parsed = match parsed_result {
            Ok(ValidatedGdtf { gdtf, .. }) => gdtf,
            Err(_) => continue,
        };

        // TODO there is an issue here related to escaping/normalizing whitespace in attributes
        // - The original file uses "&#10;" in the attribute as properly escaped new line character
        // - roxmltree properly unescapes it to "\n"
        // - quick-xml serializes it verbatim as "\n", which is a bug
        // - roxmltree properly normalizes "\n" to space
        // Track https://github.com/tafia/quick-xml/pull/379
        // Options:
        // - [x] fix up test here while serialization is broken
        // - [x] use branch from quick-xml#379 and maybe try to advance that PR
        //      -> YEAH, let's do that. If it works, comment in the PR that it worked for me.
        //      -> only fixes parsing, not serialization...
        //      -> only option would be to contribute to that PR to get it merged and later implement the fix for serialization
        // - [ ] manually serialize with https://lib.rs/crates/xml-builder
        // - [ ] try to replace quick-xml ser with https://lib.rs/crates/yaserde or https://lib.rs/crates/serde-xml-rust
        parsed.description = parsed.description.replace('\n', "");

        let serialized = serialize_gdtf(&parsed)?;
        let serialized_reader = BufReader::new(Cursor::new(serialized));
        let reparsed = parse_gdtf(serialized_reader)?;

        // dbg!(&reparsed.problems);
        assert!(
            reparsed.problems.is_empty(),
            "our serialization should not result in problems when parsed again"
        );

        assert_eq!(parsed, reparsed.gdtf);
    }
    Ok(())
}
#[test]
fn examples_roundtrip_low_level_deser_ser_deser() -> Result<(), Box<dyn Error>> {
    for (entry, file) in opened_examples_iter() {
        let mut parsed = match parse_low_level_gdtf(file) {
            Ok(ParsedGdtf { gdtf, .. }) => gdtf,
            Err(_) => continue,
        };

        // TODO there is an issue here related to escaping/normalizing whitespace in attributes
        // - The original file uses "&#10;" in the attribute as properly escaped new line character
        // - roxmltree properly unescapes it to "\n"
        // - quick-xml serializes it verbatim as "\n", which is a bug
        // - roxmltree properly normalizes "\n" to space
        // Track https://github.com/tafia/quick-xml/pull/379
        // Options:
        // - [x] fix up test here while serialization is broken
        // - [x] use branch from quick-xml#379 and maybe try to advance that PR
        //      -> YEAH, let's do that. If it works, comment in the PR that it worked for me.
        //      -> only fixes parsing, not serialization...
        //      -> only option would be to contribute to that PR to get it merged and later implement the fix for serialization
        // - [ ] manually serialize with https://lib.rs/crates/xml-builder
        // - [ ] try to replace quick-xml ser with https://lib.rs/crates/yaserde or https://lib.rs/crates/serde-xml-rust
        parsed.fixture_type.description = parsed.fixture_type.description.replace('\n', "");

        let serialized = serialize_low_level_gdtf(&parsed)?;
        let serialized_reader = BufReader::new(Cursor::new(serialized));
        let reparsed = parse_low_level_gdtf(serialized_reader)?;

        // dbg!(&reparsed.problems);
        assert!(
            reparsed.problems.is_empty(),
            "our serialization should not result in problems when parsed again"
        );

        assert_eq!(parsed, reparsed.gdtf);
    }
    Ok(())
}
