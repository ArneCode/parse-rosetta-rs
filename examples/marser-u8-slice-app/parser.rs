//! JSON over **`&[u8]`** with **borrowed slices** for number tokens and string inners (no `f64`
//! parse, no allocated `String` for those fields). Same byte-oriented rules as `marser-u8-app`.

use std::rc::Rc;

use marser::capture;
use marser::{
    matcher::{
        commit_matcher::commit_on,
        multiple::many,
        negative_lookahead,
        one_or_more::one_or_more,
        optional::optional,
    },
    one_of::one_of,
    parser::{
        deferred::recursive,
        Parser, ParserCombinator,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue<'src> {
    Null,
    Boolean(bool),
    /// Raw UTF-8 bytes of the JSON number token (same span `f64` parsing would use).
    Number(&'src [u8]),
    /// Bytes **inside** the JSON quotes (escapes appear as in the source; not decoded).
    Str(&'src [u8]),
    Array(Vec<JsonValue<'src>>),
    Object(Vec<(&'src [u8], JsonValue<'src>)>),
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
            Self::Null => "null".to_string(),
            Self::Boolean(b) => b.to_string(),
            Self::Number(bytes) => format!("number({})", String::from_utf8_lossy(bytes)),
            Self::Str(bytes) => format!(
                "str({})",
                String::from_utf8_lossy(bytes).replace('\\', "\\\\").replace('"', "\\\"")
            ),

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
                            String::from_utf8_lossy(k),
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
             => JsonValue::Number(slice)
        );

        let inner_byte = one_of((
            b' '..=b'!',
            b'#'..=b'[',
            b']'..=b'~',
            // UTF-8 non-ASCII units (JSON allows full Unicode in strings)
            b'\x80'..=b'\xff',
        ));
        let hex_digit = Rc::new(one_of((b'0'..=b'9', b'a'..=b'f', b'A'..=b'F')));
        let simple_escape = (b'\\', one_of((
            b'"', b'\\', b'/', b'b', b'f', b'n', b'r', b't',
        )));
        let unicode_escape = (
            b'\\',
            b'u',
            hex_digit.clone(),
            hex_digit.clone(),
            hex_digit.clone(),
            hex_digit.clone(),
        );

        let quoted_inner = Rc::new(capture!({
            commit_on(
                b'"',
                (
                    bind_slice!(
                        many(one_of((
                            inner_byte.clone(),
                            simple_escape,
                            unicode_escape,
                        ))),
                        inner as &'src [u8]
                    ),
                    b'"',
                    ws.clone(),
                )
            )
        } => inner));

        let string_value = quoted_inner
            .clone()
            .map_output(JsonValue::Str);

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
        } => JsonValue::Array(elements));

        let key_value_pair = Rc::new(capture!({
            (
                bind!(quoted_inner.clone(), key),
                b':',
                ws.clone(),
                bind!(element.clone(), value),
            )
        } => (key, value)));

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
        } => JsonValue::Object(key_value_pairs));

        capture!((
            ws.clone(),
            bind!(
                one_of((object, array, string_value, number, boolean, null)),
                result
            ),
            ws.clone()
        ) => result)
    })
}
