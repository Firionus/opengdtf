use std::io::Write;
use std::{
    collections::HashMap,
    fs::{self, create_dir_all, remove_dir_all, File},
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
pub static EXPECTED_TOML_PATH: Lazy<PathBuf> =
    Lazy::new(|| EXAMPLE_FILES_DIR.join("expected.toml"));

type Expected = HashMap<String, ExpectedEntry, Xxh3Builder>;

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
    Ok(ParsedInfo),
    Err(ErrorInfo),
}

impl From<Result<Parsed, Error>> for OutputEnum {
    fn from(value: Result<Parsed, Error>) -> Self {
        match value {
            Ok(parsed) => OutputEnum::Ok(ParsedInfo {
                manufacturer: parsed.gdtf.manufacturer,
                name: parsed.gdtf.name,
                fixture_type_id: parsed.gdtf.fixture_type_id.to_string(),
                problems: parsed
                    .problems
                    .into_iter()
                    .map(|p| format!("{p}"))
                    .collect(),
                geometries: {
                    let mut qualified_names = parsed
                        .gdtf
                        .geometries
                        .graph
                        .node_indices()
                        .map(|i| parsed.gdtf.geometries.qualified_name(i))
                        .collect::<Vec<String>>();
                    qualified_names.sort();
                    qualified_names
                },
            }),
            Err(e) => OutputEnum::Err(ErrorInfo {
                error: format!("{e}"),
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ErrorInfo {
    pub error: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ParsedInfo {
    pub manufacturer: String,
    pub name: String,
    pub fixture_type_id: String,
    pub problems: Vec<String>,
    pub geometries: Vec<String>,
}

pub fn parse_expected_toml() -> Expected {
    let expected_str = fs::read_to_string(&*EXPECTED_TOML_PATH).unwrap();
    toml::from_str(&expected_str).unwrap()
}

pub fn examples_iter() -> impl Iterator<Item = DirEntry> {
    WalkDir::new(&*EXAMPLES_DIR)
        .into_iter()
        .map(|e| e.unwrap())
        .filter(|e| !e.file_type().is_dir())
        .filter(|e| {
            Path::new(e.file_name())
                .extension()
                .map_or_else(|| false, |ext| ext == "gdtf")
        })
}

pub fn opened_examples_iter() -> impl Iterator<Item = (DirEntry, File)> {
    examples_iter().map(|entry| {
        let file = File::open(entry.path()).unwrap();
        (entry, file)
    })
}

pub fn parsed_examples_iter(
) -> impl Iterator<Item = (DirEntry, File, Result<Parsed, opengdtf::Error>)> {
    opened_examples_iter().map(|(entry, file)| {
        let parse_output = parse(&file);
        (entry, file, parse_output)
    })
}

pub fn examples_update_output_iter(
) -> impl Iterator<Item = (DirEntry, File, Result<Parsed, opengdtf::Error>)> {
    // clean outputs
    remove_dir_all(&*OUTPUTS_DIR).unwrap();
    create_dir_all(&*OUTPUTS_DIR).unwrap();

    parsed_examples_iter().map(|(entry, file, parse_output)| {
        let file_name = entry.file_name().to_str().unwrap();

        let mut output = File::create(OUTPUTS_DIR.join(file_name)).unwrap();
        write!(output, "{parse_output:#?}").unwrap();

        (entry, file, parse_output)
    })
}

#[allow(dead_code)] // fields accessed with Debug, which is ignored during dead code analysis
#[derive(Debug)]
struct DuplicateFilename {
    filename: String,
    number_of_occurences: u32,
}

/// Panics with diagnostic message if there are duplicate filenames in `expected`
pub fn check_for_duplicate_filenames(
    expected: HashMap<String, ExpectedEntry, xxhash_rust::xxh3::Xxh3Builder>,
) {
    let mut filename_set = HashMap::<&String, u32>::new();
    for original_filename in expected.values().map(|v| &v.filename) {
        if !filename_set.contains_key(original_filename) {
            filename_set.insert(original_filename, 1);
        } else {
            let prev_count = filename_set[original_filename];
            filename_set.insert(original_filename, prev_count + 1);
        }
    }
    let duplicates: Vec<DuplicateFilename> = filename_set
        .into_iter()
        .filter(|(_, c)| c != &1u32)
        .map(|(k, v)| DuplicateFilename {
            filename: k.to_string(),
            number_of_occurences: v,
        })
        .collect();
    assert!(
        duplicates.is_empty(),
        r"
entries with duplicate filenames:
{duplicates:#?}
This probably means a GDTF file was modified without changing its filename.
The stale entries in `expected.toml` should be removed."
    );
}
