use std::collections::BTreeMap;

use super::incremental_support::Driver;

#[test]
fn incremental_tokens_follow_fingerprints_dependencies_and_reject_old_results() {
    for crlf in [false, true] {
        let mut driver = Driver::new(crlf);
        let queries = driver.spec.oracle["initial"].clone();
        let original = driver.check(&queries, &BTreeMap::new());
        let mut previous = original.clone();
        for index in 0..driver.spec.oracle["steps"].as_array().expect("steps").len() {
            driver.change(index);
            let queries = driver.spec.oracle["steps"][index]["queries"].clone();
            previous = driver.check(&queries, &previous);
            for (file, base) in &original {
                driver.check_delta(file, base, &previous[file]);
            }
        }
        assert_eq!(
            previous, original,
            "complete repair restores streams and IDs"
        );
    }
}

#[test]
fn stale_document_versions_cannot_change_token_streams_or_ranges() {
    for crlf in [false, true] {
        let mut driver = Driver::new(crlf);
        let queries = driver.spec.oracle["initial"].clone();
        let mut previous = driver.check(&queries, &BTreeMap::new());
        driver.reject_old_versions("scripts/main.vela");
        for index in [0, 1] {
            driver.change(index);
            driver.reject_old_versions("scripts/main.vela");
            let queries = driver.spec.oracle["steps"][index]["queries"].clone();
            previous = driver.check(&queries, &previous);
        }
    }
}
