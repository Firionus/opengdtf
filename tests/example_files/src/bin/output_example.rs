use std::{env, fs::File, io::Write};

use example_files::{examples_update_output_iter, EXAMPLES_DIR, OUTPUTS_DIR};
use opengdtf::parse;

fn main() {
    let args: Vec<String> = env::args().collect();
    // let args = vec!["".to_owned(), "ARRI/ARRI@Orbiter@DMX_v5.0.gdtf".to_owned()];
    let path = EXAMPLES_DIR.to_owned().join(&args[1]);
    println!("parsing {:?}", &path);
    let file = File::open(path).unwrap();
    let parse_result = parse(&file);
    let outpath = OUTPUTS_DIR.to_owned().join(&args[1]);
    println!("writing output to {outpath:?}");
    let mut output_file = File::create(outpath).unwrap();
    write!(output_file, "{parse_result:#?}").unwrap();
}
