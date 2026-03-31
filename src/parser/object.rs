use std::collections::BTreeMap;

#[derive(Debug)]
pub enum Object {
    Number(i64),
    ByteArray(Vec<u8>),
    List(Vec<Object>),
    Dictionary(BTreeMap<Vec<u8>, Object>),
}
