use std::{collections::BTreeMap};

use crate::bencode::{
    Object, constants::{
        BYTE_ARRAY_DIVIDER, DICTIONARY_END, DICTIONARY_START, LIST_END, LIST_START, NUMBER_END,
        NUMBER_START,
    }
};

pub fn encode_object(object: &Object) -> Vec<u8> {
    match object {
        Object::Number(n) => encode_number(*n),
        Object::ByteArray(b) => encode_byte_array(b),
        Object::List(l) => encode_list(l),
        Object::Dictionary(d) => encode_dictionary(d),
    }
}

fn encode_dictionary(dictionary: &BTreeMap<Vec<u8>, Object>) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(DICTIONARY_START);
    for key in dictionary.keys() {
        bytes.extend_from_slice(&encode_byte_array(key));
        bytes.extend_from_slice(&encode_object(&dictionary[key]));
    }
    bytes.push(DICTIONARY_END);
    bytes
}

fn encode_list(list: &[Object]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(LIST_START);
    for item in list {
        bytes.extend_from_slice(&encode_object(&item));
    }
    bytes.push(LIST_END);
    bytes
}

fn encode_byte_array(byte_array: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(byte_array.len().to_string().as_bytes());
    bytes.push(BYTE_ARRAY_DIVIDER);
    bytes.extend_from_slice(&byte_array.to_vec());
    bytes
}

fn encode_number(number: i64) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(NUMBER_START);
    bytes.extend_from_slice(&mut number.to_string().as_bytes());
    bytes.push(NUMBER_END);
    bytes
}
