use std::{
    fs::{create_dir_all, remove_dir_all, File},
    io::Write,
    path::Path,
};

use example_files::examples_update_output_iter;
use opengdtf::parse;

#[test]
fn test_example_files() {
    for (entry, file, output) in examples_update_output_iter() {
        println!("{entry:?}")
    }

    // TODO list:
    // - [ ] implement expected output creation
    // - [ ] read list of expected outputs for files
    // - [x] read list of example files
    // - [x] parse GDTF
    // - [x] debug-stringify the output to a file
    // - [ ] if there is an expected output for the file, check if it matches. Otherwise fail test
    // - [ ] if there is no expected output, any problem is a test failure
    // - [ ] In case of failures, provide debug info to console and how to update the expected outputs file
    // - [ ] make output file writing optional or only do it when there is a test failure? Add evn variable for that? This might save some time during test runs
    // - [ ] Write documentation how to use in CONTRIBUTING.md
}

pub fn foo() -> String {
    "foo".to_string()
}
