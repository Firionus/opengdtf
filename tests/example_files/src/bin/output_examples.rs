use example_files::{examples_update_output_iter, OUTPUTS_DIR};

fn main() {
    println!("writing debug output for examples to {:?}", *OUTPUTS_DIR);
    for (entry, _file, _parsed_result) in examples_update_output_iter() {
        println!("{:?}", entry);
    }
}
