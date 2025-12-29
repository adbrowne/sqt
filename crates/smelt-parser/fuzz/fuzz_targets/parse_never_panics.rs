//! Fuzz target: Parse never panics
//!
//! This fuzz target verifies that the parser never panics on any input,
//! no matter how malformed or adversarial.

#![no_main]

use libfuzzer_sys::fuzz_target;
use smelt_parser::parse;

fuzz_target!(|data: &[u8]| {
    // Convert bytes to UTF-8 string (if possible)
    if let Ok(s) = std::str::from_utf8(data) {
        // Parse the string - this should NEVER panic
        let _ = parse(s);
    }
});
