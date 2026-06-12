pub(crate) const RANGE_METHODS_SOURCE: &str = r#"
fn main() {
    let total = 0;
    for tick in 0..128 {
        let start = tick;
        let end = tick + 16;
        let span = start..end;
        let empty = tick..tick;

        if span.len() != 16 || span.is_empty() || !empty.is_empty() {
            return 0;
        }

        total += span.len() + empty.len() + tick - tick;
    }
    return total;
}
"#;
