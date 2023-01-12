use xxhash_rust::xxh3::xxh3_128;

use zip::{result::ZipError, ZipArchive};

use std::fs::File;

// TODO once Seek::stream_len is stabilized, we can go to <T: Read + Seek>
// please track https://github.com/rust-lang/rust/issues/59359
/// Hash a GDTF file based on the filenames and checksums inside the archive and
/// format the output as a hex string.
pub fn gdtf_hash_string(file: File) -> Result<String, ZipError> {
    let hash = hash_gdtf(file)?;
    Ok(format!("{hash:x}"))
}

/// Hash a GDTF file based on the filenames and checksums inside the archive.
pub fn hash_gdtf(file: File) -> Result<u128, ZipError> {
    let mut zip = ZipArchive::new(file)?;
    let mut file_names: Vec<String> = zip.file_names().map(|s| s.to_string()).collect();
    file_names.sort(); // needed because zip might reorder files arbitrarily
    let mut buf = Vec::with_capacity(file_names.len() * 30);
    for file_name in file_names {
        let internal_file = zip.by_name(&file_name)?;
        buf.extend_from_slice(file_name.as_bytes());
        buf.extend_from_slice(&internal_file.crc32().to_be_bytes());
    }
    Ok(xxh3_128(&buf))
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::*;

    #[test]
    fn hash_does_not_depend_on_archive_name() {
        let archive = File::open("tests/hash_resources/archive.zip").unwrap();
        let renamed_archive = File::open("tests/hash_resources/renamed archive.zip").unwrap();
        assert_eq!(
            gdtf_hash_string(archive).unwrap(),
            gdtf_hash_string(renamed_archive).unwrap()
        )
    }

    #[test]
    fn hash_depends_on_inner_file_content() {
        let archive = File::open("tests/hash_resources/archive.zip").unwrap();
        let changed_archive =
            File::open("tests/hash_resources/archive changed file content.zip").unwrap();
        assert_ne!(
            gdtf_hash_string(archive).unwrap(),
            gdtf_hash_string(changed_archive).unwrap()
        )
    }

    #[test]
    fn hash_depends_on_inner_file_name() {
        let archive = File::open("tests/hash_resources/archive.zip").unwrap();
        let archive_renamed_inner_file =
            File::open("tests/hash_resources/archive renamed inner file.zip").unwrap();
        assert_ne!(
            gdtf_hash_string(archive).unwrap(),
            gdtf_hash_string(archive_renamed_inner_file).unwrap()
        )
    }

    #[test]
    fn hash_does_not_depend_on_inner_file_creation_date() {
        let archive = File::open("tests/hash_resources/archive.zip").unwrap();
        let archive_change_inner_creation_date =
            File::open("tests/hash_resources/archive different inner creation date.zip").unwrap();
        assert_eq!(
            gdtf_hash_string(archive).unwrap(),
            gdtf_hash_string(archive_change_inner_creation_date).unwrap()
        )
    }
}
