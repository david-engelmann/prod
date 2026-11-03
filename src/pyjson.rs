//! A JSON value model, reader, and writer that mirror the Python json
//! module closely enough for byte parity with the implementation the
//! earlier rounds pinned.
//!
//! The reader accepts what Python json.loads accepts, including
//! integers of any length (kept exact as their canonical decimal
//! text), the NaN and Infinity constants, and float literals too
//! large for the float range (read as infinity). The writer prints
//! what Python json.dumps prints, with sorted keys when asked, with
//! ensure_ascii string escaping, and with floats in the Python repr
//! form. The module also carries the Python value equality the
//! ladder's comparisons rest on and the Python repr the judgment
//! seam's refusal messages name fields with.

use std::cmp::Ordering;
use std::collections::BTreeMap;

/// One JSON value as Python would carry it after json.loads.
///
/// The derived equality is the structural one, entry order and
/// all. Python value equality, which crosses the number kinds
/// and ignores key order, is py_eq below.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    /// An integer, held as its canonical decimal text so integers of
    /// any length survive exactly, as Python ints do.
    Int(String),
    /// A float. May be infinite or NaN, as a Python float may.
    Float(f64),
    Str(String),
    List(Vec<Value>),
    /// An object. Python dicts keep insertion order and this vector
    /// does the same, with at most one entry per key (a repeated key
    /// in a parsed document keeps its first position and its last
    /// value, as a Python dict does).
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn int(value: i64) -> Value {
        Value::Int(value.to_string())
    }

    pub fn object(entries: Vec<(String, Value)>) -> Value {
        let mut out: Vec<(String, Value)> = Vec::new();
        for (key, value) in entries {
            set_key(&mut out, key, value);
        }
        Value::Object(out)
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: Value) {
        match self {
            Value::Object(entries) => set_key(entries, key.to_string(), value),
            _ => panic!("set called on a value that is not an object"),
        }
    }

    pub fn as_list(&self) -> Option<&Vec<Value>> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(text) => Some(text),
            _ => None,
        }
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }
}

fn set_key(entries: &mut Vec<(String, Value)>, key: String, value: Value) {
    for pair in entries.iter_mut() {
        if pair.0 == key {
            pair.1 = value;
            return;
        }
    }
    entries.push((key, value));
}

/// Insert a parsed key through a position index, so reading a
/// wide object costs one lookup per key rather than a scan of
/// every earlier entry. The semantics are set_key's exactly,
/// as a Python dict's are. A key's first occurrence fixes its
/// position and a later occurrence replaces the value standing
/// there.
fn set_key_indexed(
    entries: &mut Vec<(String, Value)>,
    index: &mut std::collections::HashMap<String, usize>,
    key: String,
    value: Value,
) {
    if let Some(&at) = index.get(&key) {
        entries[at].1 = value;
    } else {
        index.insert(key.clone(), entries.len());
        entries.push((key, value));
    }
}

/// Python value equality, the equality of the == operator.
///
/// Numbers compare across their kinds, exactly, so an integer and
/// the float of the same value are equal and a boolean is equal to
/// the integer one or zero it stands for. Objects compare without
/// regard to key order, as dicts do. Values of otherwise different
/// kinds never compare equal.
pub fn py_eq(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Bool(a), Value::Int(b)) => int_text_is_bool(b, *a),
        (Value::Int(a), Value::Bool(b)) => int_text_is_bool(a, *b),
        (Value::Bool(a), Value::Float(b)) => (*a as i32 as f64) == *b,
        (Value::Float(a), Value::Bool(b)) => *a == (*b as i32 as f64),
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Int(a), Value::Float(b)) => int_float_eq(a, *b),
        (Value::Float(a), Value::Int(b)) => int_float_eq(b, *a),
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| py_eq(x, y))
        }
        (Value::Object(a), Value::Object(b)) => {
            if a.len() != b.len() {
                return false;
            }
            let map: BTreeMap<&str, &Value> = b.iter().map(|(k, v)| (k.as_str(), v)).collect();
            a.iter()
                .all(|(k, v)| map.get(k.as_str()).is_some_and(|other| py_eq(v, other)))
        }
        _ => false,
    }
}

fn int_text_is_bool(text: &str, flag: bool) -> bool {
    text == if flag { "1" } else { "0" }
}

/// Whether the integer the text names equals the float, decided
/// exactly. A non finite float equals no integer, as in Python.
fn int_float_eq(text: &str, value: f64) -> bool {
    if !value.is_finite() {
        return false;
    }
    if value == 0.0 {
        return text == "0";
    }
    let (digits, negative) = match text.strip_prefix('-') {
        Some(rest) => (rest, true),
        None => (text, false),
    };
    if negative != (value < 0.0) {
        return false;
    }
    let digit_values: Vec<u8> = digits.bytes().map(|byte| byte - b'0').collect();
    cmp_decimal_to_double(&digit_values, 0, value.abs()) == Ordering::Equal
}

/// Compare two canonical integer texts as Python ints compare.
pub fn int_text_cmp(left: &str, right: &str) -> Ordering {
    let (ldigits, lneg) = split_sign(left);
    let (rdigits, rneg) = split_sign(right);
    if lneg != rneg {
        return if lneg {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    let magnitude = ldigits
        .len()
        .cmp(&rdigits.len())
        .then_with(|| ldigits.cmp(rdigits));
    if lneg { magnitude.reverse() } else { magnitude }
}

fn split_sign(text: &str) -> (&str, bool) {
    match text.strip_prefix('-') {
        Some(rest) => (rest, true),
        None => (text, false),
    }
}

/// The Python repr of a value, the form the judgment seam's
/// refusal messages name a field with.
pub fn py_repr(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(flag) => if *flag { "True" } else { "False" }.to_string(),
        Value::Int(text) => text.clone(),
        Value::Float(number) => py_float_repr(*number),
        Value::Str(text) => py_repr_str(text),
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(py_repr).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Object(entries) => {
            let parts: Vec<String> = entries
                .iter()
                .map(|(key, item)| format!("{}: {}", py_repr_str(key), py_repr(item)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

/// The Python repr of a string. The quote is a single quote unless
/// the string carries a single quote and no double quote, where it
/// is a double quote. Non printable characters stand escaped, as
/// Python escapes them. The printable test below is the one
/// Python's str.isprintable makes, over every code point.
pub fn py_repr_str(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push(quote);
    for ch in text.chars() {
        if ch == quote || ch == '\\' {
            out.push('\\');
            out.push(ch);
        } else if ch == '\n' {
            out.push_str("\\n");
        } else if ch == '\r' {
            out.push_str("\\r");
        } else if ch == '\t' {
            out.push_str("\\t");
        } else if py_printable(ch) {
            out.push(ch);
        } else {
            let point = ch as u32;
            if point <= 0xFF {
                out.push_str(&format!("\\x{:02x}", point));
            } else if point <= 0xFFFF {
                out.push_str(&format!("\\u{:04x}", point));
            } else {
                out.push_str(&format!("\\U{:08x}", point));
            }
        }
    }
    out.push(quote);
    out
}

fn py_printable(ch: char) -> bool {
    let point = ch as u32;
    if point == 0x20 {
        return true;
    }
    NON_PRINTABLE_RANGES
        .binary_search_by(|&(lo, hi)| {
            if point < lo {
                Ordering::Greater
            } else if point > hi {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        })
        .is_err()
}

/// The code point ranges Python's str.isprintable calls not
/// printable. The table was generated from the unicodedata of
/// the Python the behavioral spec runs on, Python 3.12.3 with
/// Unicode 15.0.0, by taking every code point whose general
/// category is one of the Other categories or the Separator
/// categories, the categories the isprintable rule rests on,
/// with the ASCII space excepted as the rule excepts it. The
/// probe that generated the table checked the category rule
/// against str.isprintable itself over all 1114112 code points
/// and found no disagreement.
const NON_PRINTABLE_RANGES: &[(u32, u32)] = &[
    (0x0000, 0x001F),
    (0x007F, 0x00A0),
    (0x00AD, 0x00AD),
    (0x0378, 0x0379),
    (0x0380, 0x0383),
    (0x038B, 0x038B),
    (0x038D, 0x038D),
    (0x03A2, 0x03A2),
    (0x0530, 0x0530),
    (0x0557, 0x0558),
    (0x058B, 0x058C),
    (0x0590, 0x0590),
    (0x05C8, 0x05CF),
    (0x05EB, 0x05EE),
    (0x05F5, 0x0605),
    (0x061C, 0x061C),
    (0x06DD, 0x06DD),
    (0x070E, 0x070F),
    (0x074B, 0x074C),
    (0x07B2, 0x07BF),
    (0x07FB, 0x07FC),
    (0x082E, 0x082F),
    (0x083F, 0x083F),
    (0x085C, 0x085D),
    (0x085F, 0x085F),
    (0x086B, 0x086F),
    (0x088F, 0x0897),
    (0x08E2, 0x08E2),
    (0x0984, 0x0984),
    (0x098D, 0x098E),
    (0x0991, 0x0992),
    (0x09A9, 0x09A9),
    (0x09B1, 0x09B1),
    (0x09B3, 0x09B5),
    (0x09BA, 0x09BB),
    (0x09C5, 0x09C6),
    (0x09C9, 0x09CA),
    (0x09CF, 0x09D6),
    (0x09D8, 0x09DB),
    (0x09DE, 0x09DE),
    (0x09E4, 0x09E5),
    (0x09FF, 0x0A00),
    (0x0A04, 0x0A04),
    (0x0A0B, 0x0A0E),
    (0x0A11, 0x0A12),
    (0x0A29, 0x0A29),
    (0x0A31, 0x0A31),
    (0x0A34, 0x0A34),
    (0x0A37, 0x0A37),
    (0x0A3A, 0x0A3B),
    (0x0A3D, 0x0A3D),
    (0x0A43, 0x0A46),
    (0x0A49, 0x0A4A),
    (0x0A4E, 0x0A50),
    (0x0A52, 0x0A58),
    (0x0A5D, 0x0A5D),
    (0x0A5F, 0x0A65),
    (0x0A77, 0x0A80),
    (0x0A84, 0x0A84),
    (0x0A8E, 0x0A8E),
    (0x0A92, 0x0A92),
    (0x0AA9, 0x0AA9),
    (0x0AB1, 0x0AB1),
    (0x0AB4, 0x0AB4),
    (0x0ABA, 0x0ABB),
    (0x0AC6, 0x0AC6),
    (0x0ACA, 0x0ACA),
    (0x0ACE, 0x0ACF),
    (0x0AD1, 0x0ADF),
    (0x0AE4, 0x0AE5),
    (0x0AF2, 0x0AF8),
    (0x0B00, 0x0B00),
    (0x0B04, 0x0B04),
    (0x0B0D, 0x0B0E),
    (0x0B11, 0x0B12),
    (0x0B29, 0x0B29),
    (0x0B31, 0x0B31),
    (0x0B34, 0x0B34),
    (0x0B3A, 0x0B3B),
    (0x0B45, 0x0B46),
    (0x0B49, 0x0B4A),
    (0x0B4E, 0x0B54),
    (0x0B58, 0x0B5B),
    (0x0B5E, 0x0B5E),
    (0x0B64, 0x0B65),
    (0x0B78, 0x0B81),
    (0x0B84, 0x0B84),
    (0x0B8B, 0x0B8D),
    (0x0B91, 0x0B91),
    (0x0B96, 0x0B98),
    (0x0B9B, 0x0B9B),
    (0x0B9D, 0x0B9D),
    (0x0BA0, 0x0BA2),
    (0x0BA5, 0x0BA7),
    (0x0BAB, 0x0BAD),
    (0x0BBA, 0x0BBD),
    (0x0BC3, 0x0BC5),
    (0x0BC9, 0x0BC9),
    (0x0BCE, 0x0BCF),
    (0x0BD1, 0x0BD6),
    (0x0BD8, 0x0BE5),
    (0x0BFB, 0x0BFF),
    (0x0C0D, 0x0C0D),
    (0x0C11, 0x0C11),
    (0x0C29, 0x0C29),
    (0x0C3A, 0x0C3B),
    (0x0C45, 0x0C45),
    (0x0C49, 0x0C49),
    (0x0C4E, 0x0C54),
    (0x0C57, 0x0C57),
    (0x0C5B, 0x0C5C),
    (0x0C5E, 0x0C5F),
    (0x0C64, 0x0C65),
    (0x0C70, 0x0C76),
    (0x0C8D, 0x0C8D),
    (0x0C91, 0x0C91),
    (0x0CA9, 0x0CA9),
    (0x0CB4, 0x0CB4),
    (0x0CBA, 0x0CBB),
    (0x0CC5, 0x0CC5),
    (0x0CC9, 0x0CC9),
    (0x0CCE, 0x0CD4),
    (0x0CD7, 0x0CDC),
    (0x0CDF, 0x0CDF),
    (0x0CE4, 0x0CE5),
    (0x0CF0, 0x0CF0),
    (0x0CF4, 0x0CFF),
    (0x0D0D, 0x0D0D),
    (0x0D11, 0x0D11),
    (0x0D45, 0x0D45),
    (0x0D49, 0x0D49),
    (0x0D50, 0x0D53),
    (0x0D64, 0x0D65),
    (0x0D80, 0x0D80),
    (0x0D84, 0x0D84),
    (0x0D97, 0x0D99),
    (0x0DB2, 0x0DB2),
    (0x0DBC, 0x0DBC),
    (0x0DBE, 0x0DBF),
    (0x0DC7, 0x0DC9),
    (0x0DCB, 0x0DCE),
    (0x0DD5, 0x0DD5),
    (0x0DD7, 0x0DD7),
    (0x0DE0, 0x0DE5),
    (0x0DF0, 0x0DF1),
    (0x0DF5, 0x0E00),
    (0x0E3B, 0x0E3E),
    (0x0E5C, 0x0E80),
    (0x0E83, 0x0E83),
    (0x0E85, 0x0E85),
    (0x0E8B, 0x0E8B),
    (0x0EA4, 0x0EA4),
    (0x0EA6, 0x0EA6),
    (0x0EBE, 0x0EBF),
    (0x0EC5, 0x0EC5),
    (0x0EC7, 0x0EC7),
    (0x0ECF, 0x0ECF),
    (0x0EDA, 0x0EDB),
    (0x0EE0, 0x0EFF),
    (0x0F48, 0x0F48),
    (0x0F6D, 0x0F70),
    (0x0F98, 0x0F98),
    (0x0FBD, 0x0FBD),
    (0x0FCD, 0x0FCD),
    (0x0FDB, 0x0FFF),
    (0x10C6, 0x10C6),
    (0x10C8, 0x10CC),
    (0x10CE, 0x10CF),
    (0x1249, 0x1249),
    (0x124E, 0x124F),
    (0x1257, 0x1257),
    (0x1259, 0x1259),
    (0x125E, 0x125F),
    (0x1289, 0x1289),
    (0x128E, 0x128F),
    (0x12B1, 0x12B1),
    (0x12B6, 0x12B7),
    (0x12BF, 0x12BF),
    (0x12C1, 0x12C1),
    (0x12C6, 0x12C7),
    (0x12D7, 0x12D7),
    (0x1311, 0x1311),
    (0x1316, 0x1317),
    (0x135B, 0x135C),
    (0x137D, 0x137F),
    (0x139A, 0x139F),
    (0x13F6, 0x13F7),
    (0x13FE, 0x13FF),
    (0x1680, 0x1680),
    (0x169D, 0x169F),
    (0x16F9, 0x16FF),
    (0x1716, 0x171E),
    (0x1737, 0x173F),
    (0x1754, 0x175F),
    (0x176D, 0x176D),
    (0x1771, 0x1771),
    (0x1774, 0x177F),
    (0x17DE, 0x17DF),
    (0x17EA, 0x17EF),
    (0x17FA, 0x17FF),
    (0x180E, 0x180E),
    (0x181A, 0x181F),
    (0x1879, 0x187F),
    (0x18AB, 0x18AF),
    (0x18F6, 0x18FF),
    (0x191F, 0x191F),
    (0x192C, 0x192F),
    (0x193C, 0x193F),
    (0x1941, 0x1943),
    (0x196E, 0x196F),
    (0x1975, 0x197F),
    (0x19AC, 0x19AF),
    (0x19CA, 0x19CF),
    (0x19DB, 0x19DD),
    (0x1A1C, 0x1A1D),
    (0x1A5F, 0x1A5F),
    (0x1A7D, 0x1A7E),
    (0x1A8A, 0x1A8F),
    (0x1A9A, 0x1A9F),
    (0x1AAE, 0x1AAF),
    (0x1ACF, 0x1AFF),
    (0x1B4D, 0x1B4F),
    (0x1B7F, 0x1B7F),
    (0x1BF4, 0x1BFB),
    (0x1C38, 0x1C3A),
    (0x1C4A, 0x1C4C),
    (0x1C89, 0x1C8F),
    (0x1CBB, 0x1CBC),
    (0x1CC8, 0x1CCF),
    (0x1CFB, 0x1CFF),
    (0x1F16, 0x1F17),
    (0x1F1E, 0x1F1F),
    (0x1F46, 0x1F47),
    (0x1F4E, 0x1F4F),
    (0x1F58, 0x1F58),
    (0x1F5A, 0x1F5A),
    (0x1F5C, 0x1F5C),
    (0x1F5E, 0x1F5E),
    (0x1F7E, 0x1F7F),
    (0x1FB5, 0x1FB5),
    (0x1FC5, 0x1FC5),
    (0x1FD4, 0x1FD5),
    (0x1FDC, 0x1FDC),
    (0x1FF0, 0x1FF1),
    (0x1FF5, 0x1FF5),
    (0x1FFF, 0x200F),
    (0x2028, 0x202F),
    (0x205F, 0x206F),
    (0x2072, 0x2073),
    (0x208F, 0x208F),
    (0x209D, 0x209F),
    (0x20C1, 0x20CF),
    (0x20F1, 0x20FF),
    (0x218C, 0x218F),
    (0x2427, 0x243F),
    (0x244B, 0x245F),
    (0x2B74, 0x2B75),
    (0x2B96, 0x2B96),
    (0x2CF4, 0x2CF8),
    (0x2D26, 0x2D26),
    (0x2D28, 0x2D2C),
    (0x2D2E, 0x2D2F),
    (0x2D68, 0x2D6E),
    (0x2D71, 0x2D7E),
    (0x2D97, 0x2D9F),
    (0x2DA7, 0x2DA7),
    (0x2DAF, 0x2DAF),
    (0x2DB7, 0x2DB7),
    (0x2DBF, 0x2DBF),
    (0x2DC7, 0x2DC7),
    (0x2DCF, 0x2DCF),
    (0x2DD7, 0x2DD7),
    (0x2DDF, 0x2DDF),
    (0x2E5E, 0x2E7F),
    (0x2E9A, 0x2E9A),
    (0x2EF4, 0x2EFF),
    (0x2FD6, 0x2FEF),
    (0x2FFC, 0x3000),
    (0x3040, 0x3040),
    (0x3097, 0x3098),
    (0x3100, 0x3104),
    (0x3130, 0x3130),
    (0x318F, 0x318F),
    (0x31E4, 0x31EF),
    (0x321F, 0x321F),
    (0xA48D, 0xA48F),
    (0xA4C7, 0xA4CF),
    (0xA62C, 0xA63F),
    (0xA6F8, 0xA6FF),
    (0xA7CB, 0xA7CF),
    (0xA7D2, 0xA7D2),
    (0xA7D4, 0xA7D4),
    (0xA7DA, 0xA7F1),
    (0xA82D, 0xA82F),
    (0xA83A, 0xA83F),
    (0xA878, 0xA87F),
    (0xA8C6, 0xA8CD),
    (0xA8DA, 0xA8DF),
    (0xA954, 0xA95E),
    (0xA97D, 0xA97F),
    (0xA9CE, 0xA9CE),
    (0xA9DA, 0xA9DD),
    (0xA9FF, 0xA9FF),
    (0xAA37, 0xAA3F),
    (0xAA4E, 0xAA4F),
    (0xAA5A, 0xAA5B),
    (0xAAC3, 0xAADA),
    (0xAAF7, 0xAB00),
    (0xAB07, 0xAB08),
    (0xAB0F, 0xAB10),
    (0xAB17, 0xAB1F),
    (0xAB27, 0xAB27),
    (0xAB2F, 0xAB2F),
    (0xAB6C, 0xAB6F),
    (0xABEE, 0xABEF),
    (0xABFA, 0xABFF),
    (0xD7A4, 0xD7AF),
    (0xD7C7, 0xD7CA),
    (0xD7FC, 0xF8FF),
    (0xFA6E, 0xFA6F),
    (0xFADA, 0xFAFF),
    (0xFB07, 0xFB12),
    (0xFB18, 0xFB1C),
    (0xFB37, 0xFB37),
    (0xFB3D, 0xFB3D),
    (0xFB3F, 0xFB3F),
    (0xFB42, 0xFB42),
    (0xFB45, 0xFB45),
    (0xFBC3, 0xFBD2),
    (0xFD90, 0xFD91),
    (0xFDC8, 0xFDCE),
    (0xFDD0, 0xFDEF),
    (0xFE1A, 0xFE1F),
    (0xFE53, 0xFE53),
    (0xFE67, 0xFE67),
    (0xFE6C, 0xFE6F),
    (0xFE75, 0xFE75),
    (0xFEFD, 0xFF00),
    (0xFFBF, 0xFFC1),
    (0xFFC8, 0xFFC9),
    (0xFFD0, 0xFFD1),
    (0xFFD8, 0xFFD9),
    (0xFFDD, 0xFFDF),
    (0xFFE7, 0xFFE7),
    (0xFFEF, 0xFFFB),
    (0xFFFE, 0xFFFF),
    (0x1000C, 0x1000C),
    (0x10027, 0x10027),
    (0x1003B, 0x1003B),
    (0x1003E, 0x1003E),
    (0x1004E, 0x1004F),
    (0x1005E, 0x1007F),
    (0x100FB, 0x100FF),
    (0x10103, 0x10106),
    (0x10134, 0x10136),
    (0x1018F, 0x1018F),
    (0x1019D, 0x1019F),
    (0x101A1, 0x101CF),
    (0x101FE, 0x1027F),
    (0x1029D, 0x1029F),
    (0x102D1, 0x102DF),
    (0x102FC, 0x102FF),
    (0x10324, 0x1032C),
    (0x1034B, 0x1034F),
    (0x1037B, 0x1037F),
    (0x1039E, 0x1039E),
    (0x103C4, 0x103C7),
    (0x103D6, 0x103FF),
    (0x1049E, 0x1049F),
    (0x104AA, 0x104AF),
    (0x104D4, 0x104D7),
    (0x104FC, 0x104FF),
    (0x10528, 0x1052F),
    (0x10564, 0x1056E),
    (0x1057B, 0x1057B),
    (0x1058B, 0x1058B),
    (0x10593, 0x10593),
    (0x10596, 0x10596),
    (0x105A2, 0x105A2),
    (0x105B2, 0x105B2),
    (0x105BA, 0x105BA),
    (0x105BD, 0x105FF),
    (0x10737, 0x1073F),
    (0x10756, 0x1075F),
    (0x10768, 0x1077F),
    (0x10786, 0x10786),
    (0x107B1, 0x107B1),
    (0x107BB, 0x107FF),
    (0x10806, 0x10807),
    (0x10809, 0x10809),
    (0x10836, 0x10836),
    (0x10839, 0x1083B),
    (0x1083D, 0x1083E),
    (0x10856, 0x10856),
    (0x1089F, 0x108A6),
    (0x108B0, 0x108DF),
    (0x108F3, 0x108F3),
    (0x108F6, 0x108FA),
    (0x1091C, 0x1091E),
    (0x1093A, 0x1093E),
    (0x10940, 0x1097F),
    (0x109B8, 0x109BB),
    (0x109D0, 0x109D1),
    (0x10A04, 0x10A04),
    (0x10A07, 0x10A0B),
    (0x10A14, 0x10A14),
    (0x10A18, 0x10A18),
    (0x10A36, 0x10A37),
    (0x10A3B, 0x10A3E),
    (0x10A49, 0x10A4F),
    (0x10A59, 0x10A5F),
    (0x10AA0, 0x10ABF),
    (0x10AE7, 0x10AEA),
    (0x10AF7, 0x10AFF),
    (0x10B36, 0x10B38),
    (0x10B56, 0x10B57),
    (0x10B73, 0x10B77),
    (0x10B92, 0x10B98),
    (0x10B9D, 0x10BA8),
    (0x10BB0, 0x10BFF),
    (0x10C49, 0x10C7F),
    (0x10CB3, 0x10CBF),
    (0x10CF3, 0x10CF9),
    (0x10D28, 0x10D2F),
    (0x10D3A, 0x10E5F),
    (0x10E7F, 0x10E7F),
    (0x10EAA, 0x10EAA),
    (0x10EAE, 0x10EAF),
    (0x10EB2, 0x10EFC),
    (0x10F28, 0x10F2F),
    (0x10F5A, 0x10F6F),
    (0x10F8A, 0x10FAF),
    (0x10FCC, 0x10FDF),
    (0x10FF7, 0x10FFF),
    (0x1104E, 0x11051),
    (0x11076, 0x1107E),
    (0x110BD, 0x110BD),
    (0x110C3, 0x110CF),
    (0x110E9, 0x110EF),
    (0x110FA, 0x110FF),
    (0x11135, 0x11135),
    (0x11148, 0x1114F),
    (0x11177, 0x1117F),
    (0x111E0, 0x111E0),
    (0x111F5, 0x111FF),
    (0x11212, 0x11212),
    (0x11242, 0x1127F),
    (0x11287, 0x11287),
    (0x11289, 0x11289),
    (0x1128E, 0x1128E),
    (0x1129E, 0x1129E),
    (0x112AA, 0x112AF),
    (0x112EB, 0x112EF),
    (0x112FA, 0x112FF),
    (0x11304, 0x11304),
    (0x1130D, 0x1130E),
    (0x11311, 0x11312),
    (0x11329, 0x11329),
    (0x11331, 0x11331),
    (0x11334, 0x11334),
    (0x1133A, 0x1133A),
    (0x11345, 0x11346),
    (0x11349, 0x1134A),
    (0x1134E, 0x1134F),
    (0x11351, 0x11356),
    (0x11358, 0x1135C),
    (0x11364, 0x11365),
    (0x1136D, 0x1136F),
    (0x11375, 0x113FF),
    (0x1145C, 0x1145C),
    (0x11462, 0x1147F),
    (0x114C8, 0x114CF),
    (0x114DA, 0x1157F),
    (0x115B6, 0x115B7),
    (0x115DE, 0x115FF),
    (0x11645, 0x1164F),
    (0x1165A, 0x1165F),
    (0x1166D, 0x1167F),
    (0x116BA, 0x116BF),
    (0x116CA, 0x116FF),
    (0x1171B, 0x1171C),
    (0x1172C, 0x1172F),
    (0x11747, 0x117FF),
    (0x1183C, 0x1189F),
    (0x118F3, 0x118FE),
    (0x11907, 0x11908),
    (0x1190A, 0x1190B),
    (0x11914, 0x11914),
    (0x11917, 0x11917),
    (0x11936, 0x11936),
    (0x11939, 0x1193A),
    (0x11947, 0x1194F),
    (0x1195A, 0x1199F),
    (0x119A8, 0x119A9),
    (0x119D8, 0x119D9),
    (0x119E5, 0x119FF),
    (0x11A48, 0x11A4F),
    (0x11AA3, 0x11AAF),
    (0x11AF9, 0x11AFF),
    (0x11B0A, 0x11BFF),
    (0x11C09, 0x11C09),
    (0x11C37, 0x11C37),
    (0x11C46, 0x11C4F),
    (0x11C6D, 0x11C6F),
    (0x11C90, 0x11C91),
    (0x11CA8, 0x11CA8),
    (0x11CB7, 0x11CFF),
    (0x11D07, 0x11D07),
    (0x11D0A, 0x11D0A),
    (0x11D37, 0x11D39),
    (0x11D3B, 0x11D3B),
    (0x11D3E, 0x11D3E),
    (0x11D48, 0x11D4F),
    (0x11D5A, 0x11D5F),
    (0x11D66, 0x11D66),
    (0x11D69, 0x11D69),
    (0x11D8F, 0x11D8F),
    (0x11D92, 0x11D92),
    (0x11D99, 0x11D9F),
    (0x11DAA, 0x11EDF),
    (0x11EF9, 0x11EFF),
    (0x11F11, 0x11F11),
    (0x11F3B, 0x11F3D),
    (0x11F5A, 0x11FAF),
    (0x11FB1, 0x11FBF),
    (0x11FF2, 0x11FFE),
    (0x1239A, 0x123FF),
    (0x1246F, 0x1246F),
    (0x12475, 0x1247F),
    (0x12544, 0x12F8F),
    (0x12FF3, 0x12FFF),
    (0x13430, 0x1343F),
    (0x13456, 0x143FF),
    (0x14647, 0x167FF),
    (0x16A39, 0x16A3F),
    (0x16A5F, 0x16A5F),
    (0x16A6A, 0x16A6D),
    (0x16ABF, 0x16ABF),
    (0x16ACA, 0x16ACF),
    (0x16AEE, 0x16AEF),
    (0x16AF6, 0x16AFF),
    (0x16B46, 0x16B4F),
    (0x16B5A, 0x16B5A),
    (0x16B62, 0x16B62),
    (0x16B78, 0x16B7C),
    (0x16B90, 0x16E3F),
    (0x16E9B, 0x16EFF),
    (0x16F4B, 0x16F4E),
    (0x16F88, 0x16F8E),
    (0x16FA0, 0x16FDF),
    (0x16FE5, 0x16FEF),
    (0x16FF2, 0x16FFF),
    (0x187F8, 0x187FF),
    (0x18CD6, 0x18CFF),
    (0x18D09, 0x1AFEF),
    (0x1AFF4, 0x1AFF4),
    (0x1AFFC, 0x1AFFC),
    (0x1AFFF, 0x1AFFF),
    (0x1B123, 0x1B131),
    (0x1B133, 0x1B14F),
    (0x1B153, 0x1B154),
    (0x1B156, 0x1B163),
    (0x1B168, 0x1B16F),
    (0x1B2FC, 0x1BBFF),
    (0x1BC6B, 0x1BC6F),
    (0x1BC7D, 0x1BC7F),
    (0x1BC89, 0x1BC8F),
    (0x1BC9A, 0x1BC9B),
    (0x1BCA0, 0x1CEFF),
    (0x1CF2E, 0x1CF2F),
    (0x1CF47, 0x1CF4F),
    (0x1CFC4, 0x1CFFF),
    (0x1D0F6, 0x1D0FF),
    (0x1D127, 0x1D128),
    (0x1D173, 0x1D17A),
    (0x1D1EB, 0x1D1FF),
    (0x1D246, 0x1D2BF),
    (0x1D2D4, 0x1D2DF),
    (0x1D2F4, 0x1D2FF),
    (0x1D357, 0x1D35F),
    (0x1D379, 0x1D3FF),
    (0x1D455, 0x1D455),
    (0x1D49D, 0x1D49D),
    (0x1D4A0, 0x1D4A1),
    (0x1D4A3, 0x1D4A4),
    (0x1D4A7, 0x1D4A8),
    (0x1D4AD, 0x1D4AD),
    (0x1D4BA, 0x1D4BA),
    (0x1D4BC, 0x1D4BC),
    (0x1D4C4, 0x1D4C4),
    (0x1D506, 0x1D506),
    (0x1D50B, 0x1D50C),
    (0x1D515, 0x1D515),
    (0x1D51D, 0x1D51D),
    (0x1D53A, 0x1D53A),
    (0x1D53F, 0x1D53F),
    (0x1D545, 0x1D545),
    (0x1D547, 0x1D549),
    (0x1D551, 0x1D551),
    (0x1D6A6, 0x1D6A7),
    (0x1D7CC, 0x1D7CD),
    (0x1DA8C, 0x1DA9A),
    (0x1DAA0, 0x1DAA0),
    (0x1DAB0, 0x1DEFF),
    (0x1DF1F, 0x1DF24),
    (0x1DF2B, 0x1DFFF),
    (0x1E007, 0x1E007),
    (0x1E019, 0x1E01A),
    (0x1E022, 0x1E022),
    (0x1E025, 0x1E025),
    (0x1E02B, 0x1E02F),
    (0x1E06E, 0x1E08E),
    (0x1E090, 0x1E0FF),
    (0x1E12D, 0x1E12F),
    (0x1E13E, 0x1E13F),
    (0x1E14A, 0x1E14D),
    (0x1E150, 0x1E28F),
    (0x1E2AF, 0x1E2BF),
    (0x1E2FA, 0x1E2FE),
    (0x1E300, 0x1E4CF),
    (0x1E4FA, 0x1E7DF),
    (0x1E7E7, 0x1E7E7),
    (0x1E7EC, 0x1E7EC),
    (0x1E7EF, 0x1E7EF),
    (0x1E7FF, 0x1E7FF),
    (0x1E8C5, 0x1E8C6),
    (0x1E8D7, 0x1E8FF),
    (0x1E94C, 0x1E94F),
    (0x1E95A, 0x1E95D),
    (0x1E960, 0x1EC70),
    (0x1ECB5, 0x1ED00),
    (0x1ED3E, 0x1EDFF),
    (0x1EE04, 0x1EE04),
    (0x1EE20, 0x1EE20),
    (0x1EE23, 0x1EE23),
    (0x1EE25, 0x1EE26),
    (0x1EE28, 0x1EE28),
    (0x1EE33, 0x1EE33),
    (0x1EE38, 0x1EE38),
    (0x1EE3A, 0x1EE3A),
    (0x1EE3C, 0x1EE41),
    (0x1EE43, 0x1EE46),
    (0x1EE48, 0x1EE48),
    (0x1EE4A, 0x1EE4A),
    (0x1EE4C, 0x1EE4C),
    (0x1EE50, 0x1EE50),
    (0x1EE53, 0x1EE53),
    (0x1EE55, 0x1EE56),
    (0x1EE58, 0x1EE58),
    (0x1EE5A, 0x1EE5A),
    (0x1EE5C, 0x1EE5C),
    (0x1EE5E, 0x1EE5E),
    (0x1EE60, 0x1EE60),
    (0x1EE63, 0x1EE63),
    (0x1EE65, 0x1EE66),
    (0x1EE6B, 0x1EE6B),
    (0x1EE73, 0x1EE73),
    (0x1EE78, 0x1EE78),
    (0x1EE7D, 0x1EE7D),
    (0x1EE7F, 0x1EE7F),
    (0x1EE8A, 0x1EE8A),
    (0x1EE9C, 0x1EEA0),
    (0x1EEA4, 0x1EEA4),
    (0x1EEAA, 0x1EEAA),
    (0x1EEBC, 0x1EEEF),
    (0x1EEF2, 0x1EFFF),
    (0x1F02C, 0x1F02F),
    (0x1F094, 0x1F09F),
    (0x1F0AF, 0x1F0B0),
    (0x1F0C0, 0x1F0C0),
    (0x1F0D0, 0x1F0D0),
    (0x1F0F6, 0x1F0FF),
    (0x1F1AE, 0x1F1E5),
    (0x1F203, 0x1F20F),
    (0x1F23C, 0x1F23F),
    (0x1F249, 0x1F24F),
    (0x1F252, 0x1F25F),
    (0x1F266, 0x1F2FF),
    (0x1F6D8, 0x1F6DB),
    (0x1F6ED, 0x1F6EF),
    (0x1F6FD, 0x1F6FF),
    (0x1F777, 0x1F77A),
    (0x1F7DA, 0x1F7DF),
    (0x1F7EC, 0x1F7EF),
    (0x1F7F1, 0x1F7FF),
    (0x1F80C, 0x1F80F),
    (0x1F848, 0x1F84F),
    (0x1F85A, 0x1F85F),
    (0x1F888, 0x1F88F),
    (0x1F8AE, 0x1F8AF),
    (0x1F8B2, 0x1F8FF),
    (0x1FA54, 0x1FA5F),
    (0x1FA6E, 0x1FA6F),
    (0x1FA7D, 0x1FA7F),
    (0x1FA89, 0x1FA8F),
    (0x1FABE, 0x1FABE),
    (0x1FAC6, 0x1FACD),
    (0x1FADC, 0x1FADF),
    (0x1FAE9, 0x1FAEF),
    (0x1FAF9, 0x1FAFF),
    (0x1FB93, 0x1FB93),
    (0x1FBCB, 0x1FBEF),
    (0x1FBFA, 0x1FFFF),
    (0x2A6E0, 0x2A6FF),
    (0x2B73A, 0x2B73F),
    (0x2B81E, 0x2B81F),
    (0x2CEA2, 0x2CEAF),
    (0x2EBE1, 0x2F7FF),
    (0x2FA1E, 0x2FFFF),
    (0x3134B, 0x3134F),
    (0x323B0, 0xE00FF),
    (0xE01F0, 0x10FFFF),
];

/// The Python str of a value. Strings print as themselves, integers
/// as their digits, floats in repr, and composites in their repr,
/// which is what Python str produces for a list or a dict.
pub fn py_str(value: &Value) -> String {
    match value {
        Value::Str(text) => text.clone(),
        _ => py_repr(value),
    }
}

/// A minimal big unsigned integer, little endian base 2^32, with
/// only the operations the exact float rendering below needs.
struct BigUint(Vec<u32>);

impl BigUint {
    fn from_u64(value: u64) -> BigUint {
        let mut limbs = vec![value as u32, (value >> 32) as u32];
        while limbs.len() > 1 && *limbs.last().unwrap() == 0 {
            limbs.pop();
        }
        BigUint(limbs)
    }

    fn from_digits(digits: &[u8]) -> BigUint {
        let mut out = BigUint::from_u64(0);
        for &digit in digits {
            out.mul_small(10);
            out.add_small(digit as u32);
        }
        out
    }

    fn is_zero(&self) -> bool {
        self.0.iter().all(|limb| *limb == 0)
    }

    fn mul_small(&mut self, factor: u32) {
        let mut carry: u64 = 0;
        for limb in self.0.iter_mut() {
            let step = (*limb as u64) * (factor as u64) + carry;
            *limb = step as u32;
            carry = step >> 32;
        }
        while carry > 0 {
            self.0.push(carry as u32);
            carry >>= 32;
        }
    }

    fn add_small(&mut self, value: u32) {
        let mut carry = value as u64;
        let mut index = 0;
        while carry > 0 {
            if index == self.0.len() {
                self.0.push(0);
            }
            let step = self.0[index] as u64 + carry;
            self.0[index] = step as u32;
            carry = step >> 32;
            index += 1;
        }
    }

    fn shl(&mut self, bits: usize) {
        if bits == 0 || self.is_zero() {
            return;
        }
        let rem = (bits % 32) as u32;
        if rem > 0 {
            let mut carry = 0u32;
            for limb in self.0.iter_mut() {
                let next = (*limb << rem) | carry;
                carry = *limb >> (32 - rem);
                *limb = next;
            }
            if carry > 0 {
                self.0.push(carry);
            }
        }
        let words = bits / 32;
        if words > 0 {
            let mut shifted = vec![0u32; words];
            shifted.extend_from_slice(&self.0);
            self.0 = shifted;
        }
    }

    fn divmod_small(&mut self, divisor: u32) -> u32 {
        let mut remainder: u64 = 0;
        for limb in self.0.iter_mut().rev() {
            let step = (remainder << 32) | (*limb as u64);
            *limb = (step / divisor as u64) as u32;
            remainder = step % divisor as u64;
        }
        while self.0.len() > 1 && *self.0.last().unwrap() == 0 {
            self.0.pop();
        }
        remainder as u32
    }

    fn decimal_digits(&self) -> Vec<u8> {
        if self.is_zero() {
            return vec![0];
        }
        let mut copy = BigUint(self.0.clone());
        let mut digits = Vec::new();
        while !copy.is_zero() {
            digits.push(copy.divmod_small(10) as u8);
        }
        digits.reverse();
        digits
    }

    fn cmp_big(&self, other: &BigUint) -> Ordering {
        let a = self.trimmed_len();
        let b = other.trimmed_len();
        if a != b {
            return a.cmp(&b);
        }
        for index in (0..a).rev() {
            if self.0[index] != other.0[index] {
                return self.0[index].cmp(&other.0[index]);
            }
        }
        Ordering::Equal
    }

    fn trimmed_len(&self) -> usize {
        let mut len = self.0.len();
        while len > 1 && self.0[len - 1] == 0 {
            len -= 1;
        }
        len
    }
}

/// Compare the decimal number digits times ten to the scale with
/// the exact value of a positive finite double, exactly. Digit
/// strings and binary floats alike are exact, so the comparison is
/// done with big integer arithmetic on both sides.
fn cmp_decimal_to_double(digits: &[u8], scale: i32, value: f64) -> Ordering {
    let bits = value.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i64;
    let fraction = bits & ((1u64 << 52) - 1);
    let (significand, exponent) = if biased == 0 {
        (fraction, -1074i64)
    } else {
        (fraction + (1u64 << 52), biased - 1075)
    };
    let mut left = BigUint::from_digits(digits);
    let mut right = BigUint::from_u64(significand);
    if scale >= 0 {
        for _ in 0..scale {
            left.mul_small(10);
        }
    } else {
        for _ in 0..(-scale) {
            right.mul_small(10);
        }
    }
    if exponent >= 0 {
        right.shl(exponent as usize);
    } else {
        left.shl((-exponent) as usize);
    }
    left.cmp_big(&right)
}

/// The digit string of the integer the digits name, shifted by a
/// small delta, at the same length, or None when the shift would
/// change the length.
fn shift_digit_string(digits: &[u8], delta: i32) -> Option<Vec<u8>> {
    let mut out = digits.to_vec();
    if delta > 0 {
        let mut carry = delta as u32;
        let mut index = out.len();
        while carry > 0 {
            if index == 0 {
                return None;
            }
            index -= 1;
            let step = out[index] as u32 + carry;
            out[index] = (step % 10) as u8;
            carry = step / 10;
        }
    } else if delta < 0 {
        let mut borrow = (-delta) as u32;
        let mut index = out.len();
        while borrow > 0 {
            if index == 0 {
                return None;
            }
            index -= 1;
            if out[index] as u32 >= borrow {
                out[index] -= borrow as u8;
                borrow = 0;
            } else {
                out[index] = out[index] + 10 - borrow as u8;
                borrow = 1;
            }
        }
    }
    if out[0] == 0 {
        return None;
    }
    Some(out)
}

fn parse_candidate(digits: &[u8], exponent: i32) -> f64 {
    let mut text = String::new();
    for (index, digit) in digits.iter().enumerate() {
        if index == 1 {
            text.push('.');
        }
        text.push(char::from(b'0' + digit));
    }
    text.push('e');
    text.push_str(&exponent.to_string());
    text.parse().expect("candidate parses")
}

/// The shortest digits Python repr would print for a positive
/// finite double, with the decimal exponent of the first digit.
///
/// Rust prints a shortest digit string that round trips, and so
/// does Python, but the two disagree when the exact value sits
/// exactly between two shortest candidates, where Python takes
/// the one whose last digit is even. They can also sit a digit
/// apart when the closest candidate is not the one Rust chose.
/// So the neighbors of the Rust digits that also round trip are
/// gathered, and the one closest to the exact value wins, with
/// the even rule on an exact tie, all decided exactly.
fn python_shortest_digits(magnitude: f64) -> (Vec<u8>, i32) {
    let scientific = format!("{:e}", magnitude);
    let (mantissa, exponent) = scientific.split_once('e').expect("exponent form");
    let exponent: i32 = exponent.parse().expect("integer exponent");
    let digits: Vec<u8> = mantissa
        .bytes()
        .filter(|byte| *byte != b'.')
        .map(|byte| byte - b'0')
        .collect();
    let precision = digits.len() as i32;
    let scale = exponent - (precision - 1);
    let mut pool: Vec<Vec<u8>> = vec![digits.clone()];
    for delta in [-2, -1, 1, 2] {
        if let Some(candidate) = shift_digit_string(&digits, delta)
            && parse_candidate(&candidate, exponent) == magnitude
        {
            pool.push(candidate);
        }
    }
    if pool.len() == 1 {
        return (digits, exponent);
    }
    pool.sort();
    pool.dedup();
    let mut below: Option<&Vec<u8>> = None;
    let mut above: Option<&Vec<u8>> = None;
    for candidate in &pool {
        match cmp_decimal_to_double(candidate, scale, magnitude) {
            Ordering::Equal => return (candidate.clone(), exponent),
            Ordering::Less => below = Some(candidate),
            Ordering::Greater => {
                if above.is_none() {
                    above = Some(candidate);
                }
            }
        }
    }
    let (low, high) = match (below, above) {
        (Some(low), Some(high)) => (low.clone(), high.clone()),
        (Some(_), None) => return (pool[pool.len() - 1].clone(), exponent),
        (None, Some(_)) => return (pool[0].clone(), exponent),
        (None, None) => return (digits, exponent),
    };
    // The winner is decided by which side of the midpoint of the
    // straddling pair the exact value sits on. The midpoint is
    // ten times the low candidate plus five, at one finer scale.
    let mut doubled = BigUint::from_digits(&low);
    doubled.mul_small(10);
    doubled.add_small(5);
    let midpoint = doubled.decimal_digits();
    match cmp_decimal_to_double(&midpoint, scale - 1, magnitude) {
        Ordering::Equal => {
            if low[low.len() - 1] % 2 == 0 {
                (low, exponent)
            } else {
                (high, exponent)
            }
        }
        Ordering::Greater => (low, exponent),
        Ordering::Less => (high, exponent),
    }
}

/// The Python repr of a float, which is also the form Python str
/// produces for a float and the form json.dumps writes for one.
pub fn py_float_repr(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 {
            "inf".to_string()
        } else {
            "-inf".to_string()
        };
    }
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0".to_string()
        } else {
            "0.0".to_string()
        };
    }
    let negative = value < 0.0;
    let magnitude = value.abs();
    let (digit_values, exponent) = python_shortest_digits(magnitude);
    let digits: String = digit_values
        .iter()
        .map(|digit| char::from(b'0' + digit))
        .collect();
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if !(-4..16).contains(&exponent) {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if exponent < 0 { '-' } else { '+' });
        let magnitude_exponent = exponent.abs();
        if magnitude_exponent < 10 {
            out.push('0');
        }
        out.push_str(&magnitude_exponent.to_string());
    } else {
        let point = exponent + 1;
        if point <= 0 {
            out.push_str("0.");
            for _ in 0..(-point) {
                out.push('0');
            }
            out.push_str(&digits);
        } else if point as usize >= digits.len() {
            out.push_str(&digits);
            for _ in 0..(point as usize - digits.len()) {
                out.push('0');
            }
            out.push_str(".0");
        } else {
            out.push_str(&digits[..point as usize]);
            out.push('.');
            out.push_str(&digits[point as usize..]);
        }
    }
    out
}

fn escape_string(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut buffer = [0u16; 2];
                for unit in c.encode_utf16(&mut buffer) {
                    out.push_str(&format!("\\u{:04x}", unit));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_float(value: f64, out: &mut String) {
    if value.is_nan() {
        out.push_str("NaN");
    } else if value.is_infinite() {
        out.push_str(if value > 0.0 { "Infinity" } else { "-Infinity" });
    } else {
        out.push_str(&py_float_repr(value));
    }
}

fn sorted_entries(entries: &[(String, Value)]) -> Vec<&(String, Value)> {
    let mut order: Vec<&(String, Value)> = entries.iter().collect();
    order.sort_by(|a, b| a.0.cmp(&b.0));
    order
}

fn write_compact(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Int(text) => out.push_str(text),
        Value::Float(number) => write_float(*number, out),
        Value::Str(text) => escape_string(text, out),
        Value::List(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_compact(item, out);
            }
            out.push(']');
        }
        Value::Object(entries) => {
            out.push('{');
            for (index, (key, item)) in sorted_entries(entries).iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                escape_string(key, out);
                out.push_str(": ");
                write_compact(item, out);
            }
            out.push('}');
        }
    }
}

/// Python json.dumps with sort_keys and the default separators.
pub fn dumps_compact(value: &Value) -> String {
    let mut out = String::new();
    write_compact(value, &mut out);
    out
}

fn write_indented(value: &Value, out: &mut String, level: usize) {
    match value {
        Value::List(items) if !items.is_empty() => {
            out.push_str("[\n");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&"  ".repeat(level + 1));
                write_indented(item, out, level + 1);
            }
            out.push('\n');
            out.push_str(&"  ".repeat(level));
            out.push(']');
        }
        Value::Object(entries) if !entries.is_empty() => {
            out.push_str("{\n");
            let ordered = sorted_entries(entries);
            for (index, (key, item)) in ordered.iter().enumerate() {
                if index > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&"  ".repeat(level + 1));
                escape_string(key, out);
                out.push_str(": ");
                write_indented(item, out, level + 1);
            }
            out.push('\n');
            out.push_str(&"  ".repeat(level));
            out.push('}');
        }
        _ => write_compact(value, out),
    }
}

/// Python json.dumps with sort_keys and indent 2, without the
/// trailing newline the callers add themselves.
pub fn dumps_indent2(value: &Value) -> String {
    let mut out = String::new();
    write_indented(value, &mut out, 0);
    out
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&dumps_compact(self))
    }
}

/// A parse failure. The readers name the defect themselves, so the
/// reader reports no message of its own. The too deep failure is
/// its own kind because the surfaces report it in their own words
/// and the command line answers it with a different exit status.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("the text does not parse as a JSON document")]
    Syntax,
    #[error("the document is nested too deeply")]
    TooDeep,
}

struct Parser<'a> {
    bytes: &'a [u8],
    text: &'a str,
    pos: usize,
    depth: usize,
    max_depth: usize,
}

/// The deepest container path the reader of this crate accepts
/// before refusing a document as nested too deeply, measured on
/// the Python reader the port mirrors. The trace and fixture
/// surfaces both reach a path of 9994 containers.
pub const MAX_DEPTH: usize = 9994;

/// Read a JSON document the way Python json.loads reads it.
pub fn parse(text: &str) -> Result<Value, ParseError> {
    parse_with_depth(text, MAX_DEPTH)
}

/// Read a JSON document under a ceiling on container nesting.
///
/// The Python reader recurses once per container on the deepest
/// path of a document and stops with an uncaught recursion error
/// past its ceiling, so the port refuses a document whose deepest
/// container path passes max_depth with the too deep failure
/// instead of exhausting its own stack the same way.
pub fn parse_with_depth(text: &str, max_depth: usize) -> Result<Value, ParseError> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        text,
        pos: 0,
        depth: 0,
        max_depth,
    };
    parser.skip_whitespace();
    let value = parser.read_value()?;
    parser.skip_whitespace();
    if parser.pos != parser.bytes.len() {
        return Err(ParseError::Syntax);
    }
    Ok(value)
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while let Some(byte) = self.peek() {
            if matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn read_value(&mut self) -> Result<Value, ParseError> {
        match self.peek().ok_or(ParseError::Syntax)? {
            b'{' => self.read_container(|parser| parser.read_object()),
            b'[' => self.read_container(|parser| parser.read_list()),
            b'"' => Ok(Value::Str(self.read_string()?)),
            b't' => self.read_keyword("true", Value::Bool(true)),
            b'f' => self.read_keyword("false", Value::Bool(false)),
            b'n' => self.read_keyword("null", Value::Null),
            b'N' => self.read_keyword("NaN", Value::Float(f64::NAN)),
            b'I' => self.read_keyword("Infinity", Value::Float(f64::INFINITY)),
            b'-' => {
                if self.text[self.pos..].starts_with("-Infinity") {
                    self.pos += "-Infinity".len();
                    Ok(Value::Float(f64::NEG_INFINITY))
                } else {
                    self.read_number()
                }
            }
            b'0'..=b'9' => self.read_number(),
            _ => Err(ParseError::Syntax),
        }
    }

    /// Read one container under the nesting ceiling. The root
    /// container of a document stands at depth one, so a document
    /// is refused when a container would stand deeper than the
    /// ceiling the surface reading it allows.
    fn read_container(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<Value, ParseError>,
    ) -> Result<Value, ParseError> {
        if self.depth >= self.max_depth {
            return Err(ParseError::TooDeep);
        }
        self.depth += 1;
        let value = read(self);
        self.depth -= 1;
        value
    }

    fn read_keyword(&mut self, word: &str, value: Value) -> Result<Value, ParseError> {
        if self.text[self.pos..].starts_with(word) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(ParseError::Syntax)
        }
    }

    fn read_number(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek().ok_or(ParseError::Syntax)? {
            b'0' => {
                self.pos += 1;
            }
            b'1'..=b'9' => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(ParseError::Syntax),
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(ParseError::Syntax);
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(ParseError::Syntax);
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        let literal = &self.text[start..self.pos];
        if is_float {
            literal
                .parse::<f64>()
                .map(Value::Float)
                .map_err(|_| ParseError::Syntax)
        } else if literal == "-0" {
            Ok(Value::Int("0".to_string()))
        } else {
            Ok(Value::Int(literal.to_string()))
        }
    }

    fn read_string(&mut self) -> Result<String, ParseError> {
        // The caller has seen the opening quote.
        self.pos += 1;
        let mut out = String::new();
        loop {
            let byte = self.peek().ok_or(ParseError::Syntax)?;
            match byte {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    let escaped = self.peek().ok_or(ParseError::Syntax)?;
                    self.pos += 1;
                    match escaped {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let unit = self.read_hex_unit()?;
                            if (0xD800..0xDC00).contains(&unit) {
                                // A high surrogate. Python keeps a
                                // following low surrogate as a pair,
                                // and a Rust string cannot hold a lone
                                // surrogate, so an unpaired one is read
                                // as the replacement character.
                                if self.text[self.pos..].starts_with("\\u") {
                                    let saved = self.pos;
                                    self.pos += 2;
                                    let low = self.read_hex_unit()?;
                                    if (0xDC00..0xE000).contains(&low) {
                                        let combined = 0x10000
                                            + (((unit as u32) - 0xD800) << 10)
                                            + ((low as u32) - 0xDC00);
                                        out.push(
                                            char::from_u32(combined).ok_or(ParseError::Syntax)?,
                                        );
                                    } else {
                                        self.pos = saved;
                                        out.push('\u{FFFD}');
                                    }
                                } else {
                                    out.push('\u{FFFD}');
                                }
                            } else if (0xDC00..0xE000).contains(&unit) {
                                out.push('\u{FFFD}');
                            } else {
                                out.push(char::from_u32(unit as u32).ok_or(ParseError::Syntax)?);
                            }
                        }
                        _ => return Err(ParseError::Syntax),
                    }
                }
                0x00..=0x1F => return Err(ParseError::Syntax),
                _ => {
                    // One full UTF-8 character. The text is a Rust
                    // string, so its bytes are valid UTF-8 throughout.
                    let rest = &self.text[self.pos..];
                    let ch = rest.chars().next().ok_or(ParseError::Syntax)?;
                    out.push(ch);
                    self.pos += ch.len_utf8();
                }
            }
        }
    }

    fn read_hex_unit(&mut self) -> Result<u16, ParseError> {
        let slice = self
            .text
            .get(self.pos..self.pos + 4)
            .ok_or(ParseError::Syntax)?;
        if !slice.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ParseError::Syntax);
        }
        self.pos += 4;
        u16::from_str_radix(slice, 16).map_err(|_| ParseError::Syntax)
    }

    fn read_object(&mut self) -> Result<Value, ParseError> {
        self.pos += 1;
        let mut entries: Vec<(String, Value)> = Vec::new();
        let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(entries));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(ParseError::Syntax);
            }
            let key = self.read_string()?;
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return Err(ParseError::Syntax);
            }
            self.pos += 1;
            self.skip_whitespace();
            let value = self.read_value()?;
            set_key_indexed(&mut entries, &mut index, key, value);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(entries));
                }
                _ => return Err(ParseError::Syntax),
            }
        }
    }

    fn read_list(&mut self) -> Result<Value, ParseError> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::List(items));
        }
        loop {
            self.skip_whitespace();
            let value = self.read_value()?;
            items.push(value);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::List(items));
                }
                _ => return Err(ParseError::Syntax),
            }
        }
    }
}

/// The deepest container path in a document, counted with the
/// root container at depth one and a scalar at depth zero.
///
/// The walk is iterative, on an explicit stack, so measuring a
/// document the reader accepted at its ceiling cannot itself
/// exhaust the call stack.
pub fn deepest_path(value: &Value) -> usize {
    let mut deepest = 0;
    let mut work: Vec<(&Value, usize)> = vec![(value, 0)];
    while let Some((item, depth)) = work.pop() {
        match item {
            Value::List(items) => {
                deepest = deepest.max(depth + 1);
                work.extend(items.iter().map(|child| (child, depth + 1)));
            }
            Value::Object(entries) => {
                deepest = deepest.max(depth + 1);
                work.extend(entries.iter().map(|(_, child)| (child, depth + 1)));
            }
            _ => {}
        }
    }
    deepest
}

/// The code point ranges of the Unicode decimal number category,
/// the set the argument reader's negative number patterns draw
/// their digits from. Every range starts at a digit whose value
/// is zero, so a member's value is its offset within its range,
/// in steps of ten digits per block.
const DECIMAL_DIGIT_RANGES: &[(u32, u32)] = &[
    (0x0030, 0x0039),
    (0x0660, 0x0669),
    (0x06F0, 0x06F9),
    (0x07C0, 0x07C9),
    (0x0966, 0x096F),
    (0x09E6, 0x09EF),
    (0x0A66, 0x0A6F),
    (0x0AE6, 0x0AEF),
    (0x0B66, 0x0B6F),
    (0x0BE6, 0x0BEF),
    (0x0C66, 0x0C6F),
    (0x0CE6, 0x0CEF),
    (0x0D66, 0x0D6F),
    (0x0DE6, 0x0DEF),
    (0x0E50, 0x0E59),
    (0x0ED0, 0x0ED9),
    (0x0F20, 0x0F29),
    (0x1040, 0x1049),
    (0x1090, 0x1099),
    (0x17E0, 0x17E9),
    (0x1810, 0x1819),
    (0x1946, 0x194F),
    (0x19D0, 0x19D9),
    (0x1A80, 0x1A89),
    (0x1A90, 0x1A99),
    (0x1B50, 0x1B59),
    (0x1BB0, 0x1BB9),
    (0x1C40, 0x1C49),
    (0x1C50, 0x1C59),
    (0xA620, 0xA629),
    (0xA8D0, 0xA8D9),
    (0xA900, 0xA909),
    (0xA9D0, 0xA9D9),
    (0xA9F0, 0xA9F9),
    (0xAA50, 0xAA59),
    (0xABF0, 0xABF9),
    (0xFF10, 0xFF19),
    (0x104A0, 0x104A9),
    (0x10D30, 0x10D39),
    (0x11066, 0x1106F),
    (0x110F0, 0x110F9),
    (0x11136, 0x1113F),
    (0x111D0, 0x111D9),
    (0x112F0, 0x112F9),
    (0x11450, 0x11459),
    (0x114D0, 0x114D9),
    (0x11650, 0x11659),
    (0x116C0, 0x116C9),
    (0x11730, 0x11739),
    (0x118E0, 0x118E9),
    (0x11950, 0x11959),
    (0x11C50, 0x11C59),
    (0x11D50, 0x11D59),
    (0x11DA0, 0x11DA9),
    (0x11F50, 0x11F59),
    (0x16A60, 0x16A69),
    (0x16AC0, 0x16AC9),
    (0x16B50, 0x16B59),
    (0x1D7CE, 0x1D7FF),
    (0x1E140, 0x1E149),
    (0x1E2F0, 0x1E2F9),
    (0x1E4F0, 0x1E4F9),
    (0x1E950, 0x1E959),
    (0x1FBF0, 0x1FBF9),
];

/// The decimal digit value of a character, when it is a Unicode
/// decimal digit.
pub fn decimal_digit_value(ch: char) -> Option<u32> {
    let point = ch as u32;
    DECIMAL_DIGIT_RANGES.iter().find_map(|(start, end)| {
        if point >= *start && point <= *end {
            Some((point - start) % 10)
        } else {
            None
        }
    })
}

/// The Python str of a filesystem path, the form the file
/// readers name a path with in their refusal lines.
///
/// Python holds an argument that is not valid UTF-8 as a string
/// with one surrogate escape per stray byte, and writes that
/// string to the error stream with the backslash replacement
/// error handler, so a stray byte shows as the six characters of
/// its escape. This rendering walks the same path bytes and
/// produces the same text.
pub fn py_path_display(path: &std::path::Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_os_str().as_bytes();
    let mut out = String::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        match std::str::from_utf8(rest) {
            Ok(text) => {
                out.push_str(text);
                break;
            }
            Err(error) => {
                let valid = error.valid_up_to();
                // The prefix up to valid is valid UTF-8 by the
                // error's own report.
                out.push_str(std::str::from_utf8(&rest[..valid]).unwrap_or_default());
                let bad = error.error_len().unwrap_or(rest.len() - valid);
                for byte in &rest[valid..valid + bad] {
                    out.push_str(&format!("\\udc{:02x}", byte));
                }
                rest = &rest[valid + bad..];
            }
        }
    }
    out
}
