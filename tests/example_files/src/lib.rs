use std::{
    collections::HashMap,
    fs::{self, create_dir_all, remove_dir_all, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use chrono::Utc;
use once_cell::sync::Lazy;
use opengdtf::{parse, Parsed};
use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};
use xxhash_rust::xxh3::xxh3_128;
use zip::ZipArchive;

pub static EXAMPLE_FILES_DIR: Lazy<&Path> = Lazy::new(|| Path::new("tests/example_files"));
pub static EXAMPLES_DIR: Lazy<PathBuf> = Lazy::new(|| EXAMPLE_FILES_DIR.join("examples"));
pub static OUTPUTS_DIR: Lazy<PathBuf> = Lazy::new(|| EXAMPLE_FILES_DIR.join("outputs"));

type ExpectedProblems = HashMap<String, ExpectedProblem>;

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum ExpectedProblem {
    Ok(ProblemInfo),
    Err(ErrorInfo),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorInfo {
    pub error: String,
    pub entry_created_on: chrono::DateTime<Utc>,
    pub original_filename: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProblemInfo {
    pub manufacturer: String,
    pub name: String,
    pub fixture_type_id: String,
    pub entry_created_on: chrono::DateTime<Utc>,
    pub original_filename: String,
    pub problems: Vec<String>,
}

pub static EXPECTED_PROBLEMS_PATH: Lazy<PathBuf> =
    Lazy::new(|| EXAMPLE_FILES_DIR.join("expected_problems.toml"));

pub fn parse_expected_problems() -> ExpectedProblems {
    let expected_problems_str = fs::read_to_string(&*EXPECTED_PROBLEMS_PATH).unwrap();
    toml::from_str(&expected_problems_str).unwrap()
}

pub fn hash_gdtf_file(file: File) -> String {
    let mut zip = ZipArchive::new(&file).unwrap();
    let mut buf = vec![0u8; 0];
    let mut file_names: Vec<String> = zip.file_names().map(|s| s.to_string()).collect();
    file_names.sort();
    for file_name in file_names {
        let mut internal_file = zip.by_name(&file_name).unwrap();
        internal_file.read_to_end(&mut buf).unwrap();
    }
    let hash = xxh3_128(&buf);
    format!("{hash:x}")
}

pub fn examples_update_output_iter(
) -> impl Iterator<Item = (DirEntry, File, Result<Parsed, opengdtf::Error>)> {
    // clean outputs
    remove_dir_all(&*OUTPUTS_DIR).unwrap();
    create_dir_all(&*OUTPUTS_DIR).unwrap();

    examples_iter().map(|entry| {
        let file = File::open(entry.path()).unwrap();
        let parse_output = parse(&file);

        let file_name = entry.file_name().to_str().unwrap();

        let mut output = File::create(OUTPUTS_DIR.join(file_name)).unwrap();
        write!(output, "{parse_output:#?}").unwrap();

        (entry, file, parse_output)
    })
}

pub fn examples_iter() -> impl Iterator<Item = DirEntry> {
    WalkDir::new(&*EXAMPLES_DIR)
        .into_iter()
        .map(|e| e.unwrap())
        .filter(|e| !e.file_type().is_dir())
        .filter(|e| e.file_name() != ".gitignore")
}
