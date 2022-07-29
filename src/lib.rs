use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug)]
pub struct Gdtf {
    pub data_version: String,
}

// TODO implementing TryFrom <some kind of reader or string> makes more sense, then ref that

// TODO when does TryFrom fail and when is it succesful with an error list?

// TODO implement the error list

impl TryFrom<&Path> for Gdtf {
    type Error = &'static str;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let reader = File::open(path).unwrap();
        let mut zip = zip::ZipArchive::new(reader).unwrap();
        let mut file = zip.by_name("description.xml").unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();

        let doc = roxmltree::Document::parse(&content).unwrap();

        let gdtf = Gdtf {
            data_version: doc
                .descendants()
                .find(|n| n.has_tag_name("GDTF"))
                .unwrap()
                .attribute("DataVersion")
                .unwrap()
                .into(), // TODO validate dataversion format
        };

        Ok(gdtf)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::Gdtf;

    #[test]
    fn data_version_parsing() {
        let path = Path::new("test/resources/channel_layout_test/Test@Channel_Layout_Test@v1_first_try.gdtf");
        let gdtf = Gdtf::try_from(
            path
        ).unwrap();
        assert_eq!(gdtf.data_version, "1.1");
    }
}
