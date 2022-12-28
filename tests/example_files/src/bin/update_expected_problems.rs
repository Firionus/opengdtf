use std::{collections::HashMap, io::Read};

use example_files::examples_update_output_iter;
use serde::{Deserialize, Serialize};

use xxhash_rust::xxh3::xxh3_128;
use zip::ZipArchive;

type ExpectedProblems = HashMap<String, ExpectedProblem>;

#[derive(Serialize, Deserialize, Debug)]
struct ExpectedProblem {
    name: String,
}

fn main() {
    // TODO deserialize instead, so that we will add onto it only the changed parts
    let mut expected_problems = ExpectedProblems::new();

    for (entry, file, output) in examples_update_output_iter() {
        println!("{entry:?}");

        let mut zip = ZipArchive::new(&file).unwrap();
        let mut buf = vec![0u8; 0];
        let mut file_names: Vec<String> = zip.file_names().map(|s| s.to_string()).collect();
        file_names.sort();
        for file_name in file_names {
            let mut internal_file = zip.by_name(&file_name).unwrap();
            internal_file.read_to_end(&mut buf).unwrap();
        }
        let hash = xxh3_128(&buf);
        let key = format!("{hash:x}");

        expected_problems.insert(
            key,
            ExpectedProblem {
                name: output.unwrap().gdtf.name,
            },
        );
    }
    let serialized = toml::to_string(&expected_problems).unwrap();
    println!("{serialized}")

    // TODO validate there are no duplicates based on xml metadata, warn otherwise
}
