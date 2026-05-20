//! JSON benchmark binary over **`&[u8]`** input (`Token = &u8`).
//!
//! Crate / binary: `marser-u8-app`.
//!
//! String interiors use **byte** rules (ASCII-oriented); they are not full
//! Unicode codepoint validation. Suitable for fixtures like `canada.json`.

mod parser;

use std::{env, fs, process};

use marser::parser::Parser;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: marser-u8-app <path-to-json-file>");
        process::exit(2);
    });
    let bytes = fs::read(&path).unwrap_or_else(|e| {
        eprintln!("Failed to read '{path}': {e}");
        process::exit(1);
    });
    let p = parser::get_json_grammar();
    match p.parse_whole_input(bytes.as_slice()) {
        Ok((value, _errors)) => {
            #[cfg(debug_assertions)]
            {
                println!("{}", value.serialize_pretty());
            }
            #[cfg(not(debug_assertions))]
            {
                std::hint::black_box(value);
            }
        }
        Err(err) => {
            eprintln!("{}: {}", path, err);
            process::exit(1);
        }
    }
}
