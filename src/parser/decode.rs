use std::collections::BTreeMap;

use crate::parser::Object;

const NUMBER_START: u8 = b'i';
const NUMBER_END: u8 = b'e';
const LIST_START: u8 = b'l';
const LIST_END: u8 = b'e';
const DICTIONARY_START: u8 = b'd';
const DICTIONARY_END: u8 = b'e';
const BYTE_ARRAY_DIVIDER: u8 = b':';

pub fn decode_object(bytes: &[u8]) -> Object {
    let mut iter = bytes.iter().copied().peekable();
    decode(&mut iter)
}

fn decode<I>(iter: &mut std::iter::Peekable<I>) -> Object
where
    I: Iterator<Item = u8>,
{
    return match iter.peek() {
        Some(&b) if b == DICTIONARY_START => decode_dictionary(iter),
        Some(&b) if b == LIST_START => decode_list(iter),
        Some(&b) if b == NUMBER_START => decode_number(iter),
        _ => decode_byte_array(iter),
    };
}

fn decode_dictionary<I>(iter: &mut std::iter::Peekable<I>) -> Object
where
    I: Iterator<Item = u8>,
{
    assert_eq!(iter.next(), Some(DICTIONARY_START));

    let mut dictionary = BTreeMap::new();

    while let Some(&b) = iter.peek() {
        if b == DICTIONARY_END {
            iter.next();
            break;
        }

        if let Object::ByteArray(key) = decode_byte_array(iter) {
            let value = decode(iter);
            dictionary.insert(key, value);
        } else {
            panic!("invalid dictionary key");
        }
    }

    Object::Dictionary(dictionary)
}

fn decode_list<I>(iter: &mut std::iter::Peekable<I>) -> Object
where
    I: Iterator<Item = u8>,
{
    let mut list = Vec::new();

    while let Some(&b) = iter.peek() {
        if b == LIST_END {
            iter.next();
            break;
        }
        list.push(decode(iter))
    }

    Object::List(list)
}

fn decode_number<I>(iter: &mut std::iter::Peekable<I>) -> Object
where
    I: Iterator<Item = u8>,
{
    assert_eq!(iter.next(), Some(NUMBER_START));

    let mut bytes = Vec::new();
    let mut found_end = false;

    while let Some(b) = iter.next() {
        if b == NUMBER_END {
            found_end = true;
            break;
        }
        bytes.push(b);
    }

    if !found_end {
        panic!("Number missing terminating 'e'");
    }

    let num_str = str::from_utf8(&bytes).unwrap();

    if num_str.len() > 1 && num_str.starts_with('0') {
        panic!("Leading zeros are not allowed");
    }

    let number: i64 = num_str.parse().unwrap();
    Object::Number(number)
}

fn decode_byte_array<I>(iter: &mut std::iter::Peekable<I>) -> Object
where
    I: Iterator<Item = u8>,
{
    let mut length_bytes = Vec::new();

    while let Some(b) = iter.next() {
        if b == BYTE_ARRAY_DIVIDER {
            break;
        }
        length_bytes.push(b);
    }

    let length_str = str::from_utf8(&length_bytes).unwrap();

    if length_str.len() > 1 && length_str.starts_with('0') {
        panic!("Leading zeros are now allowed");
    }

    let length: usize = length_str.parse().unwrap();

    let mut bytes = Vec::new();

    for _ in 0..length {
        match iter.next() {
            Some(b) => bytes.push(b),
            None => panic!("Unexpected end of input when reading byte array"),
        }
    }

    assert_eq!(length, bytes.len());

    Object::ByteArray(bytes)
}
