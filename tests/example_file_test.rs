use std::{
    fs::{create_dir_all, remove_dir_all, File},
    io::Write,
    path::Path,
};

use opengdtf::parse;
use walkdir::WalkDir;

#[test]
fn test_example_files() {
    let example_files_dir = Path::new("tests/resources/example_files");
    let examples_dir = example_files_dir.join("examples");
    let outputs_dir = example_files_dir.join("outputs");

    // clean outputs
    remove_dir_all(&outputs_dir).unwrap();
    create_dir_all(&outputs_dir).unwrap();

    for entry in WalkDir::new(examples_dir)
        .into_iter()
        .map(|e| e.unwrap())
        .filter(|e| !e.file_type().is_dir())
        .filter(|e| e.file_name() != ".gitignore")
    {
        let file = File::open(entry.path()).unwrap();
        let parse_output = parse(file);

        let file_name = entry.file_name().to_str().unwrap();

        let mut output = File::create(outputs_dir.join(file_name)).unwrap();
        write!(output, "{parse_output:#?}").unwrap();
    }

    // TODO list:
    // - [ ] read list of expected outputs for files
    // - [x] read list of example files
    // - [ ] for each file run its own smoke test:
    //  - [x] parse it
    //  - [x] debug-stringify the output to a file
    //  - [ ] if there are problems, check if the output matches expected output
    // - [ ] In case of failures, provide debug info to console and how to update the expected outputs file
    // - [ ] Write documentation how to use in CONTRIBUTING.md
}
