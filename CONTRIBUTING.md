:construction: This file for contributors is a work in progress. If you can't find something here you want to know, do
not hesitate even a second to open an issue: https://github.com/Firionus/opengdtf/issues/new :relaxed:

## Example File Tests

### Setup

Running the tests for this library requires manually downloading some GDTF files from gdtf-share.com:

1. Download GDTF files  
   Log into GDTF Share and go to https://gdtf-share.com/share.php?page=downloadFiles. Check "latest revision" and then
   download the files for the Manufacturers "Robe Lighting", "ARRI" and "Ayrton".
2. Extract GDTF files and place in `tests/example_files/examples`
3. Run `cargo test`  
   The test results should tell you whether all expected fixtures are present and whether expected results exist for
   every fixture. If these tests fail, you should not feel too bad. Likely, nothing is broken, you just have a slightly
   different set of example files than the last developer.  
   The more important test is `expected_toml_matches_examples_output`. If there are differences here, there is a change
   in the library compared to the library version with which the example file tests were last updated.

### Commands

Force update of `tests/example_files/expected.toml` (will never
overwrite fixtures whose outputs stays the same or are absent from your examples): 
```sh
cargo run --bin update_expected
``` 

Dump debug output of examples files to `tests/example_files/outputs`: 
```
cargo run --bin output_examples
```

### Background

This library has to ensure it works well with the output of the official GDTF Builder, which is not always standard
conformant. Further, when this library evolves, we should have a mechanism to detect regressions with a wide range of
real world examples. 

This is currently achieved with the example file testing, which is a modified form of [snapshot
testing](https://notlaura.com/what-is-a-snapshot-test/), where the output of tests is serialized and commited to the
repository for future comparison. This ensures changes are in the future are detected. 

Since I do not want to host a large array of GDTF files in this repository (due to licensing and size concerns), only
very few demo files I created myself are commited in the repository. Other files have to be downloaded by each developer
individually. This doesn't scale well to many developers, but for the moment it's what we have.

The expected output for the tests is saved in `tests/example_files/expected.toml`. Matching the example files to the
expected output is achieved with a hash of the extracted filenames and file contents in the GDTF archive. 
