use std::collections::BTreeMap;

use super::lifecycle_support::Driver;

#[test]
fn source_tokens_follow_overlay_close_save_and_dependency_lifecycle() {
    for crlf in [false, true] {
        let mut driver = Driver::new("semantic-token-source-lifecycle", crlf);
        let states = driver.spec.oracle["states"]
            .as_array()
            .expect("states")
            .clone();
        let mut original = BTreeMap::new();
        let mut previous = BTreeMap::new();
        for (index, state) in states.iter().enumerate() {
            driver.apply(state);
            driver.assert_sources(state);
            previous = driver.check(&state["queries"], &previous);
            if index == 0 {
                original = previous.clone();
            } else {
                for (file, base) in &original {
                    driver.check_delta(file, base, &previous[file]);
                }
            }
        }
        assert_eq!(previous, original, "repair restores streams and IDs");
    }
}
