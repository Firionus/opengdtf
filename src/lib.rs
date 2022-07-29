use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug)]
pub struct Gdtf {
    pub data_version: String,
}

// TODO when does TryFrom fail and when is it succesful with an error list?

// TODO implement the error list

// TODO replace unwraps by non-panicking code

impl TryFrom<&str> for Gdtf {
    type Error = &'static str;

    fn try_from(description_content: &str) -> Result<Self, Self::Error> {
        let doc = roxmltree::Document::parse(&description_content).unwrap();

        let gdtf = Gdtf {
            data_version: doc
                .descendants()
                .find(|n| n.has_tag_name("GDTF"))
                .unwrap()
                .attribute("DataVersion")
                .unwrap()
                .into(), // TODO validate DataVersion format
        };

        Ok(gdtf)
    }
}

impl TryFrom<&String> for Gdtf {
    type Error = &'static str;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        Gdtf::try_from(&value[..])
    }
}

impl TryFrom<&Path> for Gdtf {
    type Error = &'static str;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let reader = File::open(path).unwrap();
        let mut zip = zip::ZipArchive::new(reader).unwrap();
        let mut file = zip.by_name("description.xml").unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();

        Ok(Gdtf::try_from(&content[..]).unwrap())
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
