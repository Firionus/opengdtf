use opengdtf::Gdtf;
use std::path::Path;

fn main() {
    println!("This parses a GDTF file and outputs the result to the console");

    let path =
        Path::new("test/resources/channel_layout_test/Test@Channel_Layout_Test@v1_first_try.gdtf");

    print_gdtf(path);

    print_gdtf(Path::new("test/resources/Robe_Lighting@Robin_Tetra2@04062021.gdtf"))
}

fn print_gdtf(path: &Path) {
    let gdtf = Gdtf::try_from(path).unwrap();
    println!("{:#?}", gdtf);
    gdtf.problems.iter().for_each(|problem| println!("{}", problem));
}
