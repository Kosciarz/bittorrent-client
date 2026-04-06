use std::collections::BTreeMap;

use crate::bencode::{
    Object,
    constants::{
        BYTE_ARRAY_DIVIDER, DICTIONARY_END, DICTIONARY_START, LIST_END, LIST_START, NINE_BYTE,
        NUMBER_END, NUMBER_START, ZERO_BYTE,
    },
};

#[derive(Debug)]
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn peek(&mut self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    fn position(&self) -> usize {
        self.pos
    }
}

pub fn decode_object(bytes: &[u8]) -> Object {
    let mut cursor = Cursor::new(bytes);
    decode(&mut cursor)
}

fn decode(cursor: &mut Cursor) -> Object {
    return match cursor.peek() {
        Some(DICTIONARY_START) => decode_dictionary(cursor),
        Some(LIST_START) => decode_list(cursor),
        Some(NUMBER_START) => decode_number(cursor),
        Some(ZERO_BYTE..=NINE_BYTE) => decode_byte_array(cursor),
        _ => panic!("Invalid token"),
    };
}

fn decode_key(cursor: &mut Cursor) -> Vec<u8> {
    match decode_byte_array(cursor) {
        Object::ByteArray(b) => b,
        _ => panic!("Expected byte array for dictionary key"),
    }
}

fn decode_dictionary(cursor: &mut Cursor) -> Object {
    assert_eq!(cursor.next().expect("Expected 'd'"), DICTIONARY_START);

    let mut dict = BTreeMap::new();

    while let Some(b) = cursor.peek() {
        if b == DICTIONARY_END {
            cursor.next();
            break;
        }

        let key = decode_key(cursor);
        let value = decode(cursor);

        dict.insert(key, value);
    }

    Object::Dictionary(dict)
}

fn decode_list(cursor: &mut Cursor) -> Object {
    assert_eq!(cursor.next().expect("Expected 'l'"), LIST_START);

    let mut list = Vec::new();

    while let Some(b) = cursor.peek() {
        if b == LIST_END {
            cursor.next();
            break;
        }
        list.push(decode(cursor))
    }

    Object::List(list)
}

fn read_until(cursor: &mut Cursor, terminator: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    while let Some(b) = cursor.next() {
        if b == terminator {
            break;
        }
        bytes.push(b);
    }
    bytes
}

fn parse_and_check_for_leading_zeros(bytes: &[u8]) -> &str {
    let num_str = str::from_utf8(&bytes).unwrap();
    if num_str.len() > 1 && num_str.starts_with('0') {
        panic!("Leading zeros are not allowed");
    }
    num_str
}

fn decode_number(cursor: &mut Cursor) -> Object {
    assert_eq!(cursor.next().expect("Expected 'i'"), NUMBER_START);

    let bytes = read_until(cursor, NUMBER_END);
    let num_str = parse_and_check_for_leading_zeros(&bytes);
    let num: i64 = num_str.parse().unwrap();

    Object::Number(num)
}

fn decode_byte_array(cursor: &mut Cursor) -> Object {
    let len_bytes = read_until(cursor, BYTE_ARRAY_DIVIDER);
    let length_str = parse_and_check_for_leading_zeros(&len_bytes);
    let len: usize = length_str.parse().unwrap();

    let mut bytes = Vec::new();

    for _ in 0..len {
        match cursor.next() {
            Some(b) => bytes.push(b),
            None => panic!("Unexpected end of input when reading byte array"),
        }
    }

    assert_eq!(len, bytes.len());

    Object::ByteArray(bytes)
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     // --- decode_number ---

//     #[test]
//     fn decode_number_works() {
//         let bytes = b"i123456789e";
//         let mut iter = bytes.iter().copied().peekable();

//         if let Object::Number(num) = decode_number(&mut iter) {
//             assert_eq!(num, 123456789);
//         }
//     }

//     #[test]
//     fn decode_number_zero_works() {
//         let bytes = b"i0e";
//         let mut iter = bytes.iter().copied().peekable();

//         if let Object::Number(num) = decode_number(&mut iter) {
//             assert_eq!(num, 0);
//         }
//     }

//     #[test]
//     fn decode_number_negative_works() {
//         let bytes = b"i-123456789e";
//         let mut iter = bytes.iter().copied().peekable();

//         if let Object::Number(num) = decode_number(&mut iter) {
//             assert_eq!(num, -123456789);
//         }
//     }

//     #[test]
//     #[should_panic]
//     fn decode_number_with_leading_zeros_panics() {
//         let bytes = b"i0123456789e";
//         let mut iter = bytes.iter().copied().peekable();

//         decode_number(&mut iter);
//     }

//     #[test]
//     #[should_panic]
//     fn decode_number_empty_input_panics() {
//         let bytes = b"";
//         let mut iter = bytes.iter().copied().peekable();

//         decode_number(&mut iter);
//     }

//     #[test]
//     #[should_panic]
//     fn decode_number_missing_i_panics() {
//         let bytes = b"123456789e";
//         let mut iter = bytes.iter().copied().peekable();

//         decode_number(&mut iter);
//     }

//     #[test]
//     #[should_panic]
//     fn decode_number_only_i_panics() {
//         let bytes = b"i";
//         let mut iter = bytes.iter().copied().peekable();

//         decode_number(&mut iter);
//     }

//     #[test]
//     #[should_panic]
//     fn decode_number_missing_e_panics() {
//         let bytes = b"i123456789";
//         let mut iter = bytes.iter().copied().peekable();

//         decode_number(&mut iter);
//     }

//     #[test]
//     #[should_panic]
//     fn decode_number_invalid_number_panics() {
//         let bytes = b"i123x56789e";
//         let mut iter = bytes.iter().copied().peekable();

//         decode_number(&mut iter);
//     }

//     // --- decody_byte_array ---

//     #[test]
//     fn decode_byte_array_works() {
//         let bytes = b"14:example string";
//         let mut iter = bytes.iter().copied().peekable();

//         if let Object::ByteArray(byte_array) = decode_byte_array(&mut iter) {
//             assert_eq!(byte_array, b"example string");
//         }
//     }

//     #[test]
//     fn decode_byte_array_zero_length_works() {
//         let bytes = b"0:";
//         let mut iter = bytes.iter().copied().peekable();

//         if let Object::ByteArray(byte_array) = decode_byte_array(&mut iter) {
//             assert_eq!(byte_array, b"");
//         }
//     }

//     #[test]
//     #[should_panic]
//     fn decode_byte_array_input_too_long_panics() {
//         let bytes = b"16:example string";
//         let mut iter = bytes.iter().copied().peekable();

//         decode_byte_array(&mut iter);
//     }

//     #[test]
//     #[should_panic]
//     fn decode_byte_array_input_too_short_panics() {
//         let bytes = b"10:abc";
//         let mut iter = bytes.iter().copied().peekable();

//         decode_byte_array(&mut iter);
//     }

//     #[test]
//     #[should_panic]
//     fn decode_byte_array_length_with_characters_panics() {
//         let bytes = b"1x:example string";
//         let mut iter = bytes.iter().copied().peekable();

//         decode_byte_array(&mut iter);

//         let bytes = b"xx:example string";
//         let mut iter = bytes.iter().copied().peekable();

//         decode_byte_array(&mut iter);
//     }

//     #[test]
//     #[should_panic]
//     fn decode_byte_array_length_with_leading_zeros_panics() {
//         let bytes = b"014:example string";
//         let mut iter = bytes.iter().copied().peekable();

//         decode_byte_array(&mut iter);
//     }

//     #[test]
//     #[should_panic]
//     fn decode_byte_array_no_divider_panics() {
//         let bytes = b"14example string";
//         let mut iter = bytes.iter().copied().peekable();

//         decode_byte_array(&mut iter);
//     }

//     // --- decode_list ---

//     #[test]
//     fn decode_list_works() {
//         todo!()
//     }

//     // --- decode_dictionary ---

//     #[test]
//     fn decode_dictionary_works() {
//         todo!()
//     }
// }
