use std::{env, error::Error, fs, path::Path};

use crate::{bencode::encode_object};

mod bencode;
mod torrent;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        panic!("Invalid argument count");
    }

    let path = args[1].to_string();
    let path = Path::new(&path);

    let bytes = fs::read(path)?;
    let object = bencode::decode_object(&bytes);

    assert_eq!(encode_object(&object), bytes);

    Ok(())
}
