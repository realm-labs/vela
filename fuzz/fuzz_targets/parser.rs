#![no_main]

use libfuzzer_sys::fuzz_target;
use vela_common::SourceId;
use vela_syntax::parse::parse_source_with_id;

fuzz_target!(|source: &str| {
    let parsed = parse_source_with_id(SourceId::new(1), source);
    drop(parsed);
});
