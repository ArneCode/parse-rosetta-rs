//! Minimal JSON grammar over **`&[u8]`** (`Token = &u8`), mirroring `marser-bare-app`.
//!
//! String contents are matched **per byte** (ASCII-safe for typical benchmarks; not full UTF-8
//! string semantics).

use std::rc::Rc;

use marser::capture;
use marser::{
    matcher::{
        commit_matcher::commit_on,
        multiple::many,
        negative_lookahead,
        one_or_more::one_or_more,
        optional::optional,
        positive_lookahead,
    },
    one_of::one_of,
    parser::{
        deferred::recursive,
        token_parser::TokenParser,
        Parser, ParserCombinator,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue<'src> {
    Invalid(&'src [u8]),
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue<'src>>),
    Object(Vec<(String, JsonValue<'src>)>),
}

impl<'src> JsonValue<'src> {
    pub fn serialize_pretty(&self) -> String {
        self.serialize_internal(0)
    }

    fn serialize_internal(&self, indent_level: usize) -> String {
        let indent_size = 4;
        let current_indent = " ".repeat(indent_level * indent_size);
        let nested_indent = " ".repeat((indent_level + 1) * indent_size);

        match self {
            Self::Invalid(slice) => format!("invalid('{}')", String::from_utf8_lossy(slice)),
            Self::Null => "null".to_string(),
            Self::Boolean(b) => b.to_string(),
            Self::Number(n) => n.to_string(),
            Self::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),

            Self::Array(arr) => {
                if arr.is_empty() {
                    return "[]".to_string();
                }
                let items: Vec<String> = arr
                    .iter()
                    .map(|v| {
                        format!(
                            "{}{}",
                            nested_indent,
                            v.serialize_internal(indent_level + 1)
                        )
                    })
                    .collect();
                format!("[\n{}\n{current_indent}]", items.join(",\n"))
            }

            Self::Object(obj) => {
                if obj.is_empty() {
                    return "{}".to_string();
                }
                let pairs: Vec<String> = obj
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "{}\"{}\": {}",
                            nested_indent,
                            k,
                            v.serialize_internal(indent_level + 1)
                        )
                    })
                    .collect();
                format!("{{\n{}\n{current_indent}}}", pairs.join(",\n"))
            }
        }
    }
}

pub fn get_json_grammar<'src>(
) -> impl Parser<'src, &'src [u8], Output = JsonValue<'src>> + Clone {
    recursive(|element| {
        let ws = Rc::new(many(one_of((b' ', b'\t', b'\n', b'\r'))));

        let null = capture!((b"null".as_slice(), ws.clone()) => JsonValue::Null);
        let bool_false = capture!((b"false".as_slice(), ws.clone()) => JsonValue::Boolean(false));
        let bool_true = capture!((b"true".as_slice(), ws.clone()) => JsonValue::Boolean(true));
        let boolean = one_of((bool_true, bool_false));
        let number = capture!(
            bind_slice!((
                optional(b'-'),
                one_of((
                    b'0',
                    (b'1'..=b'9', many(b'0'..=b'9'))
                )),
                optional((b'.', one_or_more(b'0'..=b'9'))),
                optional((
                    one_of((b'e', b'E')),
                    optional(one_of((b'+', b'-'))),
                    one_or_more(b'0'..=b'9')
                )),
                negative_lookahead(one_of((
                    b'+',
                    b'-',
                    b'0'..=b'9',
                    b'.',
                    b'e',
                    b'E',
                )))
            ), slice as &'src [u8])
             => {
                let s = std::str::from_utf8(slice).unwrap_or("");
                JsonValue::Number(s.parse().unwrap_or(0.0))
            }
        );

        let character = Rc::new(TokenParser::new(
            |t: &&u8| **t != b'"' && **t != b'\\' && **t >= 0x20,
            |t: &&u8| char::from(**t),
        ));
        let hex_digit = Rc::new(one_of((b'0'..=b'9', b'a'..=b'f', b'A'..=b'F')));
        let escaped_char = capture!({
            (
                b'\\',
                bind!(
                    one_of((
                        b'"',
                        b'\\',
                        b'/',
                        b'b',
                        b'f',
                        b'n',
                        b'r',
                        b't',
                    )),
                    esc
                )
            )
        } => {
            match esc {
                b'"' => '"',
                b'\\' => '\\',
                b'/' => '/',
                b'b' => '\u{0008}',
                b'f' => '\u{000C}',
                b'n' => '\n',
                b'r' => '\r',
                b't' => '\t',
                _ => '\u{FFFD}',
            }
        });
        let unicode_escape = capture!({
            (
                b'\\',
                b'u',
                bind!(hex_digit.clone(), d0),
                bind!(hex_digit.clone(), d1),
                bind!(hex_digit.clone(), d2),
                bind!(hex_digit.clone(), d3),
            )
        } => {
            let hex: String = [d0, d1, d2, d3].iter().map(|&b| b as char).collect();
            let codepoint = u32::from_str_radix(&hex, 16).unwrap_or(0xFFFD);
            std::char::from_u32(codepoint).unwrap_or('\u{FFFD}')
        });
        let raw_string = Rc::new(capture!({
            commit_on(
                b'"',(
                many(one_of((
                    bind!(character.clone(), *chars),
                    bind!(escaped_char, *chars),
                    bind!(unicode_escape, *chars),
                ))),
                b'"',
                ws.clone()
            ))
        } =>  {
            chars.into_iter().collect::<String>()
        }));

        let array = capture!({
            commit_on((ws.clone(), b'['),
            (
                ws.clone(),
                optional((
                    bind!(element.clone(), *elements),
                    many((b',', ws.clone(), bind!(element.clone(), *elements))),
                )),
                ws.clone(),
                b']',
                ws.clone()
            ))
        } =>  {
            JsonValue::Array(elements)
        });

        let key_value_pair = Rc::new(capture!({
            (
                bind!(raw_string.clone(), key),
                b':',
                ws.clone(),
                bind!(element.clone(), value),
            )
        } => {
            (key, value)
        }));

        let object = capture!({
            commit_on((ws.clone(), b'{'),
            (
                ws.clone(),
                optional((
                    bind!(key_value_pair.clone(), *key_value_pairs),
                    many((b',', ws.clone(), bind!(key_value_pair.clone(), *key_value_pairs))),
                )),
                b'}',
                ws.clone()
            ))
        } => {
            JsonValue::Object(key_value_pairs)
        });

        let string = raw_string.map_output(JsonValue::String);

        capture!((
            ws.clone(),
            bind!(
                one_of((object, array, string, number, boolean, null)),
                result
            ),
            ws.clone()
        ) => result)
    })
}
