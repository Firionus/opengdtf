use example_files::{examples_update_output_iter, hash::hash_gdtf, parse_expected_problems};

#[test]
fn test_example_files() {
    let expected_problems = parse_expected_problems();

    for (_entry, file, _output) in examples_update_output_iter() {
        let key = hash_gdtf(file);
        match expected_problems.get(&key) {
            Some(_) => todo!(),
            None => {
                todo!()
            }
        }
    }

    // TODO list:
    // - [x] implement expected output creation
    // - [x] read list of expected outputs for files
    // - [x] read list of example files
    // - [x] parse GDTF
    // - [x] debug-stringify the output to a file
    // - [x] make tests for the properties of the GDTF hash (original little zip file, one with changed creation date, one with changed file name, one with changed content)
    //       hash for GDTF files should include filenames as well as file content (but not file metadata, like last changed date)
    // - [x] when re-running update, existing entries should only be updated when something changes. Currently, the timestamp is updated, creating unnecessary diffs.
    // - [x] ordering of entries in expected.toml should be deterministic based on file hash
    // - [x] comment field in expected problems (default empty string, to be filled by people, should be copied when replacing entry with new output)
    // - [x] Problems should stay consistent even when problem stringification changes, no? Maybe only assert the type of problem??? -> let's assert on debug output
    // - [x] deterministic Geometry renaming
    // - [x] how to ensure that Geometries stay the same in the future?
    //       Just number of Geometries? Or names of geometries? Or qualified names of Geometries to also capture relations (best idea so far)?
    //       Just using a hash is not a good idea, because it doesn't communicate to the dev what went wrong.
    // - [ ] make separate test functions for: (so they can independently fail and report their issues)
    //       no file-name duplicates in expected.toml;
    //       all entries in expected.toml are present in examples (to avoid rot of fixture files no one has anymore);
    //       all examples are present in expected.toml (to avoid errors that were not checked by a real person);
    //       check that output of matches between expected.toml and examples are the same
    // - [ ] make output file writing optional or only do it when there is a test failure? Add even variable for that? This might save some time during test runs
    // - [ ] refactor and clean up variable names, they are super messy
    // - [ ] Write documentation how to use in CONTRIBUTING.md
}

pub fn foo() -> String {
    "foo".to_string()
}
