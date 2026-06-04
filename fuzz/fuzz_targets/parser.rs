#![no_main]

use libfuzzer_sys::fuzz_target;
use vela_common::SourceId;
use vela_syntax::parser::parse_source;

fuzz_target!(|source: &str| {
    let parsed = parse_source(SourceId::new(1), source);
    drop(parsed);
});
