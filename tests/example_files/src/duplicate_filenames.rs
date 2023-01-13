use std::collections::HashMap;

use crate::ExpectedEntry;

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
    let mut filename_counts = HashMap::<&String, u32>::new();
    for original_filename in expected.values().map(|v| &v.filename) {
        if let Some(prev_count) = filename_counts.get_mut(original_filename) {
            *prev_count += 1
        } else {
            filename_counts.insert(original_filename, 1);
        };
    }
    let duplicates: Vec<DuplicateFilename> = filename_counts
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
