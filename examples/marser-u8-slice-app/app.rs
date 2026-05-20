//! JSON benchmark binary over **`&[u8]`** with **borrowed** number and string payloads.
//!
//! String values and object keys store the **lexical inner** bytes between quotes (escapes are
//! still present as in the source). Numbers store the matched **token slice** without `f64`
//! parsing.

mod parser;

use std::{env, fs, process};

use marser::parser::Parser;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: marser-u8-slice-app <path-to-json-file>");
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
