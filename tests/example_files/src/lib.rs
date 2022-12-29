pub mod hash;

use std::{
    collections::HashMap,
    fs::{self, create_dir_all, remove_dir_all, File},
    io::Write,
    path::{Path, PathBuf},
};

use chrono::Utc;
use once_cell::sync::Lazy;
use opengdtf::{parse, Error, Parsed};
use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};
use xxhash_rust::xxh3::Xxh3Builder;

pub static EXAMPLE_FILES_DIR: Lazy<&Path> = Lazy::new(|| Path::new("tests/example_files"));
pub static EXAMPLES_DIR: Lazy<PathBuf> = Lazy::new(|| EXAMPLE_FILES_DIR.join("examples"));
pub static OUTPUTS_DIR: Lazy<PathBuf> = Lazy::new(|| EXAMPLE_FILES_DIR.join("outputs"));

type ExpectedProblems = HashMap<String, ExpectedEntry, Xxh3Builder>;

#[derive(Serialize, Deserialize, Debug)]
pub struct ExpectedEntry {
    pub filename: String,
    pub saved_on: chrono::DateTime<Utc>,
    pub comment: String,
    #[serde(flatten)]
    pub output_enum: OutputEnum,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum OutputEnum {
    Ok(ProblemInfo),
    Err(ErrorInfo),
}

impl From<Result<Parsed, Error>> for OutputEnum {
    fn from(value: Result<Parsed, Error>) -> Self {
        match value {
            Ok(parsed) => OutputEnum::Ok(ProblemInfo {
                manufacturer: parsed.gdtf.manufacturer,
                name: parsed.gdtf.name,
                fixture_type_id: parsed.gdtf.fixture_type_id.to_string(),
                problems: parsed
                    .problems
                    .into_iter()
                    .map(|p| format!("{p:?}"))
                    .collect(),
            }),
            Err(e) => OutputEnum::Err(ErrorInfo {
                error: format!("{e:?}"),
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ErrorInfo {
    pub error: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ProblemInfo {
    pub manufacturer: String,
    pub name: String,
    pub fixture_type_id: String,
    pub problems: Vec<String>,
}

pub static EXPECTED_PROBLEMS_PATH: Lazy<PathBuf> =
    Lazy::new(|| EXAMPLE_FILES_DIR.join("expected_problems.toml"));

pub fn parse_expected_problems() -> ExpectedProblems {
    let expected_problems_str = fs::read_to_string(&*EXPECTED_PROBLEMS_PATH).unwrap();
    toml::from_str(&expected_problems_str).unwrap()
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
