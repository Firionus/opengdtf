use std::{
    fs::{create_dir_all, remove_dir_all, File},
    io::Write,
    path::{Path, PathBuf},
};

use once_cell::sync::Lazy;
use opengdtf::{parse, Parsed};
use walkdir::{DirEntry, WalkDir};

pub static EXAMPLE_FILES_DIR: Lazy<&Path> = Lazy::new(|| Path::new("tests/example_files"));
pub static EXAMPLES_DIR: Lazy<PathBuf> = Lazy::new(|| EXAMPLE_FILES_DIR.join("examples"));
pub static OUTPUTS_DIR: Lazy<PathBuf> = Lazy::new(|| EXAMPLE_FILES_DIR.join("outputs"));

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
