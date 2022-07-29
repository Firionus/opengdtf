use opengdtf::Gdtf;
use std::path::Path;

fn main() {
    println!("This parses a GDTF file and outputs the result the console");

    let path =
        Path::new("test/resources/channel_layout_test/Test@Channel_Layout_Test@v1_first_try.gdtf");
    let gdtf = Gdtf::try_from(path).unwrap();
    println!("{:?}", gdtf)
}
