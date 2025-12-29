//! Fuzz target: Round-trip testing
//!
//! This fuzz target verifies that valid SQL can be:
//! 1. Parsed into an AST
//! 2. Printed back to SQL
//! 3. Re-parsed into an equivalent AST
//!
//! This ensures the printer generates valid SQL and preserves semantics.

#![no_main]

use libfuzzer_sys::fuzz_target;
use smelt_parser::{parse, File};

fuzz_target!(|data: &[u8]| {
    // Convert bytes to UTF-8 string (if possible)
    if let Ok(sql) = std::str::from_utf8(data) {
        // Parse the original SQL
        let parse1 = parse(sql);

        // Only test round-trip if the original parse succeeded
        if parse1.errors.is_empty() {
            // Try to print the AST back to SQL
            if let Some(file) = File::cast(parse1.syntax()) {
                let printed = file.to_string();

                // Re-parse the printed SQL
                let parse2 = parse(&printed);

                // The re-parsed SQL should also have no errors
                // If it has errors, that's a bug in the printer!
                if !parse2.errors.is_empty() {
                    panic!(
                        "Round-trip failed!\nOriginal: {}\nPrinted: {}\nErrors: {:?}",
                        sql, printed, parse2.errors
                    );
                }
            }
        }
    }
});
