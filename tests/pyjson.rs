//! PR-35 and PR-38. The Python compatible JSON layer, pinned
//! against the forms the Python reader and writer produce.

mod common;

use common::value;
use prod::pyjson::{
    self, ParseError, Value, deepest_path, dumps_compact, dumps_indent2, int_text_cmp, py_eq,
    py_float_repr, py_repr,
};

#[test]
fn integers_of_any_length_read_exactly() {
    let parsed = pyjson::parse("123456789012345678901234567890").unwrap();
    assert_eq!(
        parsed,
        Value::Int("123456789012345678901234567890".to_string())
    );
    assert_eq!(dumps_compact(&parsed), "123456789012345678901234567890");
    let negative_zero = pyjson::parse("-0").unwrap();
    assert_eq!(negative_zero, Value::Int("0".to_string()));
}

#[test]
fn the_python_constants_read_as_python_reads_them() {
    assert!(matches!(
        pyjson::parse("NaN").unwrap(),
        Value::Float(number) if number.is_nan()
    ));
    assert_eq!(
        pyjson::parse("Infinity").unwrap(),
        Value::Float(f64::INFINITY)
    );
    assert_eq!(
        pyjson::parse("-Infinity").unwrap(),
        Value::Float(f64::NEG_INFINITY)
    );
    let overflow = pyjson::parse("1e400").unwrap();
    assert_eq!(overflow, Value::Float(f64::INFINITY));
}

#[test]
fn a_repeated_key_keeps_its_first_position_and_last_value() {
    let parsed = pyjson::parse(r#"{"a": 1, "b": 2, "a": 3}"#).unwrap();
    assert_eq!(dumps_compact(&parsed), r#"{"a": 3, "b": 2}"#);
}

#[test]
fn an_unpaired_surrogate_reads_as_the_replacement_character() {
    let parsed = pyjson::parse(r#""\ud800""#).unwrap();
    assert_eq!(parsed, Value::Str("\u{FFFD}".to_string()));
    let pair = pyjson::parse(r#""\ud83d\ude00""#).unwrap();
    assert_eq!(pair, Value::Str("\u{1F600}".to_string()));
}

#[test]
fn malformed_documents_are_syntax_failures() {
    for text in ["", "{", "[1,", "tru", "{'a': 1}", "01", "1.", ".5", "+1"] {
        assert!(
            matches!(pyjson::parse(text), Err(ParseError::Syntax)),
            "text {:?} is refused",
            text
        );
    }
}

#[test]
fn floats_print_in_the_python_repr_form() {
    let cases = [
        (0.82, "0.82"),
        (0.35, "0.35"),
        (1.0, "1.0"),
        (0.6, "0.6"),
        (0.0001, "0.0001"),
        (0.00001, "1e-05"),
        (1e16, "1e+16"),
        (1234567890123456.0, "1234567890123456.0"),
        (2.5e-7, "2.5e-07"),
        (-0.0, "-0.0"),
        (0.1 + 0.2, "0.30000000000000004"),
    ];
    for (number, expected) in cases {
        assert_eq!(py_float_repr(number), expected, "repr of {}", number);
    }
}

#[test]
fn the_writer_escapes_as_python_with_ensure_ascii() {
    let parsed = value(r#"{"b": "café", "a": [1, 2.5, "x\ny"]}"#);
    assert_eq!(
        dumps_compact(&parsed),
        "{\"a\": [1, 2.5, \"x\\ny\"], \"b\": \"caf\\u00e9\"}"
    );
    assert_eq!(
        dumps_indent2(&parsed),
        "{\n  \"a\": [\n    1,\n    2.5,\n    \"x\\ny\"\n  ],\n  \"b\": \"caf\\u00e9\"\n}"
    );
    let empty = value(r#"{"list": [], "object": {}}"#);
    assert_eq!(
        dumps_indent2(&empty),
        "{\n  \"list\": [],\n  \"object\": {}\n}"
    );
}

#[test]
fn value_equality_crosses_the_number_kinds_as_python_does() {
    assert!(py_eq(&value("1"), &value("1.0")));
    assert!(py_eq(&value("true"), &value("1")));
    assert!(!py_eq(&value("1"), &value("1.5")));
    assert!(py_eq(
        &value(r#"{"a": 1, "b": [2]}"#),
        &value(r#"{"b": [2.0], "a": 1}"#)
    ));
    assert!(!py_eq(&value(r#""1""#), &value("1")));
    assert!(!py_eq(&value("null"), &value("0")));
}

#[test]
fn repr_names_values_as_python_repr_does() {
    assert_eq!(py_repr(&value(r#""plain""#)), "'plain'");
    assert_eq!(py_repr(&value(r#""it's""#)), "\"it's\"");
    assert_eq!(py_repr(&value("7")), "7");
    assert_eq!(py_repr(&value("true")), "True");
    assert_eq!(py_repr(&value("null")), "None");
    assert_eq!(py_repr(&value(r#"[1, "a"]"#)), "[1, 'a']");
}

#[test]
fn integer_texts_compare_as_integers() {
    use std::cmp::Ordering;
    assert_eq!(int_text_cmp("9", "10"), Ordering::Less);
    assert_eq!(int_text_cmp("-10", "-9"), Ordering::Less);
    assert_eq!(
        int_text_cmp(
            "123456789012345678901234567890",
            "123456789012345678901234567891"
        ),
        Ordering::Less
    );
    assert_eq!(int_text_cmp("42", "42"), Ordering::Equal);
}

#[test]
fn the_deepest_path_counts_containers_from_the_root() {
    assert_eq!(deepest_path(&value("1")), 0);
    assert_eq!(deepest_path(&value("[]")), 1);
    assert_eq!(deepest_path(&value(r#"{"a": [[1]]}"#)), 3);
}

#[test]
fn the_reader_refuses_a_document_past_its_ceiling() {
    // The reader recurses once per container, as the Python
    // reader does, so the check runs on a thread with a stack
    // sized for the ceiling, the same device the binary uses.
    let handle = std::thread::Builder::new()
        .stack_size(512 * 1024 * 1024)
        .spawn(|| {
            let mut text = String::new();
            for _ in 0..9994 {
                text.push('[');
            }
            for _ in 0..9994 {
                text.push(']');
            }
            assert!(pyjson::parse(&text).is_ok());
            let mut deeper = String::from("[");
            deeper.push_str(&text);
            deeper.push(']');
            assert!(matches!(pyjson::parse(&deeper), Err(ParseError::TooDeep)));
        })
        .expect("thread spawns");
    handle.join().expect("the ceiling check passes");
}
