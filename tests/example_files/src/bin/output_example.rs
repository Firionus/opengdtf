use std::{env, fs::File, io::Write};

use example_files::{EXAMPLES_DIR, OUTPUTS_DIR};
use opengdtf::parse;

fn main() {
    let args: Vec<String> = env::args().collect();
    // let args = vec![
    //     "".to_owned(),
    //     "Robe_Lighting@Robin_Tetra2@2022-12-05__Geometry_names_revision.gdtf".to_owned(),
    // ];
    let path = EXAMPLES_DIR.to_owned().join(&args[1]);
    println!("parsing {:?}", &path);
    let file = File::open(path).unwrap();
    let parse_result = parse(&file);
    let outpath = OUTPUTS_DIR.to_owned().join(&args[1]);
    println!("writing output to {outpath:?}");
    let mut output_file = File::create(outpath).unwrap();
    write!(output_file, "{parse_result:#?}").unwrap();
}
