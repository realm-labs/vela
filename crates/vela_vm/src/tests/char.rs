use super::*;

#[test]
fn char_methods_match_rust_char_semantics() {
    assert_eq!(
        run_standard_source(
            r#"
fn main() {
    if '奖'.to_string() == "奖"
        && ' '.is_whitespace()
        && '7'.is_ascii()
        && '7'.is_ascii_digit()
        && !'奖'.is_ascii()
    {
        return true;
    }
    return false;
}
"#
        ),
        Ok(OwnedValue::Bool(true))
    );
}
