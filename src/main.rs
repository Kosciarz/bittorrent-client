use std::{env, path::Path};

mod parser;

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        panic!("Invalid argument count");
    }

    let path = args[1].to_string();
    let path = Path::new(&path);
    let object = parser::decode_file(path);

    dbg!(object);
}
