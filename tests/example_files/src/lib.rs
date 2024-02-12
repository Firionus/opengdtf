//! TODO we could probably use the GDTF Share API to make the example file stuff more easy to set up for new contributors
use std::collections::BTreeMap;
use std::io::Write;
use std::{
    fs::{self, create_dir_all, remove_dir_all, File},
    path::{Path, PathBuf},
};

mod duplicate_filenames;
pub use duplicate_filenames::check_for_duplicate_filenames;

use chrono::Utc;
use once_cell::sync::Lazy;
use opengdtf::{parse_gdtf, Error, Gdtf, ValidatedGdtf};
use serde::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

pub static EXAMPLE_FILES_DIR: Lazy<&Path> = Lazy::new(|| Path::new("tests/example_files"));
pub static EXAMPLES_DIR: Lazy<PathBuf> = Lazy::new(|| EXAMPLE_FILES_DIR.join("examples"));
pub static OUTPUTS_DIR: Lazy<PathBuf> = Lazy::new(|| EXAMPLE_FILES_DIR.join("outputs"));
pub static EXPECTED_TOML_PATH: Lazy<PathBuf> =
    Lazy::new(|| EXAMPLE_FILES_DIR.join("expected.toml"));

type Expected = BTreeMap<String, ExpectedEntry>;

#[derive(Serialize, Deserialize, Debug)]
pub struct ExpectedEntry {
    pub filename: String,
    pub saved_on: chrono::DateTime<Utc>,
    pub comment: String,
    #[serde(flatten)]
    pub output_enum: OutputEnum,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum OutputEnum {
    Ok(ParsedInfo),
    Err(ErrorInfo),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ErrorInfo {
    pub error: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ParsedInfo {
    #[serde(flatten)]
    pub gdtf: Gdtf,
    pub problems: Vec<String>,
}

impl From<Result<ValidatedGdtf, Error>> for OutputEnum {
    fn from(value: Result<ValidatedGdtf, Error>) -> Self {
        match value {
            Ok(parsed) => OutputEnum::Ok(ParsedInfo {
                gdtf: parsed.gdtf,
                problems: {
                    let mut problem_strings: Vec<String> = parsed
                        .problems
                        .into_iter()
                        .map(|p| format!("{p}"))
                        .collect();
                    problem_strings.sort();
                    problem_strings
                },
            }),
            Err(err) => OutputEnum::Err(ErrorInfo {
                error: format!("{err}"),
            }),
        }
    }
}

pub fn parse_expected_toml() -> Expected {
    let expected_str = fs::read_to_string(&*EXPECTED_TOML_PATH).unwrap();
    toml::from_str(&expected_str).unwrap()
}

pub fn examples_iter() -> impl Iterator<Item = DirEntry> {
    WalkDir::new(&*EXAMPLES_DIR)
        .into_iter()
        .map(|result| result.unwrap())
        .filter(|entry| !entry.file_type().is_dir())
        .filter(|entry| {
            Path::new(entry.file_name())
                .extension()
                .map_or_else(|| false, |extension| extension == "gdtf")
        })
}

pub fn opened_examples_iter() -> impl Iterator<Item = (DirEntry, File)> {
    examples_iter().map(|entry| {
        let file = File::open(entry.path()).unwrap();
        (entry, file)
    })
}

pub fn parsed_examples_iter(
) -> impl Iterator<Item = (DirEntry, File, Result<ValidatedGdtf, opengdtf::Error>)> {
    opened_examples_iter().map(|(entry, file)| {
        let parse_result = parse_gdtf(&file);
        (entry, file, parse_result)
    })
}

pub fn examples_update_output_iter(
) -> impl Iterator<Item = (DirEntry, File, Result<ValidatedGdtf, opengdtf::Error>)> {
    // clean outputs
    remove_dir_all(&*OUTPUTS_DIR).unwrap();
    create_dir_all(&*OUTPUTS_DIR).unwrap();

    parsed_examples_iter().map(|(entry, file, parse_result)| {
        let file_name = entry.file_name().to_str().unwrap();

        let mut output_file = File::create(OUTPUTS_DIR.join(file_name)).unwrap();
        write!(output_file, "{parse_result:#?}").unwrap();

        (entry, file, parse_result)
    })
}
