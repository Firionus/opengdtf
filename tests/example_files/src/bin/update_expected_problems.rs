use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::{collections::HashMap, fs, io::Read};

use chrono::Utc;
use example_files::examples_update_output_iter;
use example_files::EXAMPLE_FILES_DIR;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use xxhash_rust::xxh3::xxh3_128;
use zip::ZipArchive;

type ExpectedProblems = HashMap<String, ExpectedProblem>;

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum ExpectedProblem {
    Ok(ProblemInfo),
    Err(ErrorInfo),
}

#[derive(Serialize, Deserialize, Debug)]
struct ErrorInfo {
    error: String,
    entry_created_on: chrono::DateTime<Utc>,
    original_filename: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ProblemInfo {
    manufacturer: String,
    name: String,
    fixture_type_id: String,
    entry_created_on: chrono::DateTime<Utc>,
    original_filename: String,
    problems: Vec<String>,
}

static EXPECTED_PROBLEMS_PATH: Lazy<PathBuf> =
    Lazy::new(|| EXAMPLE_FILES_DIR.join("expected_problems.toml"));

fn main() {
    let expected_problems_str = fs::read_to_string(&*EXPECTED_PROBLEMS_PATH).unwrap();
    let mut expected_problems: ExpectedProblems = toml::from_str(&expected_problems_str).unwrap();

    println!("iterating over example files");
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

        let info = match output {
            Ok(parsed) => ExpectedProblem::Ok(ProblemInfo {
                manufacturer: parsed.gdtf.manufacturer,
                name: parsed.gdtf.name,
                fixture_type_id: parsed.gdtf.fixture_type_id.to_string(),
                original_filename: format!("{}", entry.file_name().to_string_lossy()),
                problems: parsed.problems.into_iter().map(|p| p.to_string()).collect(),
                entry_created_on: Utc::now(),
            }),
            Err(e) => ExpectedProblem::Err(ErrorInfo {
                error: e.to_string(),
                entry_created_on: Utc::now(),
                original_filename: format!("{:?}", entry.file_name()),
            }),
        };

        expected_problems.insert(key, info);
    }
    let serialized = toml::to_string_pretty(&expected_problems).unwrap();
    let mut output_file = File::create(&*EXPECTED_PROBLEMS_PATH).unwrap();
    write!(output_file, "{}", &serialized).unwrap();

    // check for duplicate original filenames
    let mut filename_set = HashMap::<&String, u32>::new();
    for original_filename in expected_problems.values().map(|v| match v {
        ExpectedProblem::Ok(p) => &p.original_filename,
        ExpectedProblem::Err(i) => &i.original_filename,
    }) {
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
