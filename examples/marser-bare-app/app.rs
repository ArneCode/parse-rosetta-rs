//! Same CLI as `marser-app`, but uses the **bare** JSON grammar (no rich error-handling wrappers).
//! Compare wall time against `marser-app` to see how much comes from error UX combinators.
//!
//! Crate / binary: `marser-bare-app`.

mod parser;

use std::{env, fs, process};

use marser::parser::Parser;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: marser-bare-app <path-to-json-file>");
        process::exit(2);
    });
    let src = fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("Failed to read '{path}': {e}");
        process::exit(1);
    });
    let p = parser::get_json_grammar();
    match p.parse_str(src.as_str()) {
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
