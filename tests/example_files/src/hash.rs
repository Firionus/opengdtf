use xxhash_rust::xxh3::xxh3_128;

use zip::ZipArchive;

use std::io::{Read, Seek};

pub fn hash_gdtf<T: Read + Seek>(file: T) -> String {
    let mut zip = ZipArchive::new(file).unwrap();
    let mut buf = vec![0u8; 0];
    let mut file_names: Vec<String> = zip.file_names().map(|s| s.to_string()).collect();
    file_names.sort(); // needed because zip might reorder files arbitrarily
    for file_name in file_names {
        let mut internal_file = zip.by_name(&file_name).unwrap();
        buf.extend_from_slice(file_name.as_bytes());
        internal_file.read_to_end(&mut buf).unwrap();
    }
    let hash = xxh3_128(&buf);
    format!("{hash:x}")
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::*;

    #[test]
    fn hash_does_not_depend_on_archive_name() {
        let archive = File::open("tests/resources/archive.zip").unwrap();
        let renamed_archive = File::open("tests/resources/renamed archive.zip").unwrap();
        assert_eq!(hash_gdtf(archive), hash_gdtf(renamed_archive))
    }

    #[test]
    fn hash_depends_on_inner_file_content() {
        let archive = File::open("tests/resources/archive.zip").unwrap();
        let changed_archive =
            File::open("tests/resources/archive changed file content.zip").unwrap();
        assert_ne!(hash_gdtf(archive), hash_gdtf(changed_archive))
    }

    #[test]
    fn hash_depends_on_inner_file_name() {
        let archive = File::open("tests/resources/archive.zip").unwrap();
        let archive_renamed_inner_file =
            File::open("tests/resources/archive renamed inner file.zip").unwrap();
        assert_ne!(hash_gdtf(archive), hash_gdtf(archive_renamed_inner_file))
    }

    #[test]
    fn hash_does_not_depend_on_inner_file_creation_date() {
        let archive = File::open("tests/resources/archive.zip").unwrap();
        let archive_change_inner_creation_date =
            File::open("tests/resources/archive different inner creation date.zip").unwrap();
        assert_eq!(
            hash_gdtf(archive),
            hash_gdtf(archive_change_inner_creation_date)
        )
    }
}
