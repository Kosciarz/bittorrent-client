use std::{fs, path::Path};

use crate::parser::{Object, decode::decode_object};

pub fn decode_file(path: &Path) -> Object {
    let bytes = fs::read(path).unwrap();
    decode_object(&bytes)
}
