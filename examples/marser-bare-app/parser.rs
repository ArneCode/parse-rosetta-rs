//! Minimal JSON grammar for **ablation**: same shape as `marser-app` but without
//! `with_label`, `add_error_info`, `recover_with`, `try_insert_if_missing`, `if_error` /
//! `unwanted`, `if_error_else_fail`, `erase_types_in_debug`, or the `invalid_element` arm.

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
    Invalid(&'src str),
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
            Self::Invalid(slice) => format!("invalid('{slice}')"),
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

pub fn get_json_grammar<'src>() -> impl Parser<'src, &'src str, Output = JsonValue<'src>> + Clone {
    recursive(|element| {
        let ws = Rc::new(many(one_of((' ', '\t', '\n', '\r'))));

        let null = capture!(("null", ws.clone()) => JsonValue::Null);
        let bool_false = capture!(("false", ws.clone()) => JsonValue::Boolean(false));
        let bool_true = capture!(("true", ws.clone()) => JsonValue::Boolean(true));
        let boolean = one_of((bool_true, bool_false));
        let number = capture!(
            commit_on(positive_lookahead(one_of(('-', '.', '+', '0'..='9'))),
            bind_slice!((
                optional('-'),
                one_of((
                    '0',
                    ('1'..='9',many('0'..='9'))
                )),
                optional((
                    '.', one_or_more('0'..='9')
                )),
                optional((
                    one_of(('e', 'E')),
                    optional(one_of(('+', '-'))),
                    one_or_more('0'..='9')
                )),
                negative_lookahead(one_of(('+','-','0'..='9','.','e','E')))
            ), slice as &'src str))
             => {
                JsonValue::Number(slice.parse().unwrap_or(0.0))
            }
        );

        let character = Rc::new(TokenParser::new(
            |c| *c != '"' && *c != '\\' && (*c as u32) >= 0x20,
            |x| *x,
        ));
        let hex_digit = Rc::new(one_of(('0'..='9', 'a'..='f', 'A'..='F')));
        let escaped_char = capture!({
            (
                '\\',
                bind!(one_of(('\"', '\\', '/', 'b', 'f', 'n', 'r', 't')), esc)
            )
        } => {
            match esc {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'b' => '\u{0008}',
                'f' => '\u{000C}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                _ => esc,
            }
        });
        let unicode_escape = capture!({
            (
                '\\', 'u',
                bind!(hex_digit.clone(), *digits),
                bind!(hex_digit.clone(), *digits),
                bind!(hex_digit.clone(), *digits),
                bind!(hex_digit.clone(), *digits)
            )
        } => {
            let hex: String = digits.into_iter().collect();
            let codepoint = u32::from_str_radix(&hex, 16).unwrap_or(0xFFFD);
            std::char::from_u32(codepoint).unwrap_or('\u{FFFD}')
        });
        let raw_string = Rc::new(capture!({
            commit_on(
                '"',(
                many(one_of((
                    bind!(character.clone(), *chars),
                    bind!(escaped_char, *chars),
                    bind!(unicode_escape, *chars),
                ))),
                '"',
                ws.clone()
            ))
        } =>  {
            chars.into_iter().collect::<String>()
        }));

        let array = capture!({
            commit_on((ws.clone(), '['),
            (
                ws.clone(),
                optional((
                    bind!(element.clone(), *elements),
                    many((',', ws.clone(), bind!(element.clone(), *elements))),
                )),
                ws.clone(),
                ']',
                ws.clone()
            ))
        } =>  {
            JsonValue::Array(elements)
        });

        let key_value_pair = Rc::new(capture!({
            (
                bind!(raw_string.clone(), key),
                ':',
                ws.clone(),
                bind!(element.clone(), value),
            )
        } => {
            (key, value)
        }));

        let object = capture!({
            commit_on((ws.clone(), '{'),
            (
                ws.clone(),
                optional((
                    bind!(key_value_pair.clone(), *key_value_pairs),
                    many((',', ws.clone(), bind!(key_value_pair.clone(), *key_value_pairs))),
                )),
                '}',
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
