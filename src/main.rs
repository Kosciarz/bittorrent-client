use std::{env, fs, path::Path};

use crate::bencode::encode_object;

mod bencode;

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        panic!("Invalid argument count");
    }

    let path = args[1].to_string();
    let path = Path::new(&path);
    let object = bencode::decode_file(path);

    let bytes = fs::read(path).unwrap();
    assert_eq!(encode_object(&object), bytes);
}
