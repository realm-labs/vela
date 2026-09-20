use super::{Spec, load};

pub(crate) fn cases(crlf: bool) -> Vec<(String, bool, Spec)> {
    let catalog = load("rename-declaration-collisions");
    catalog.oracle["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .map(|case| {
            let mut spec = catalog.clone();
            spec.files.insert(
                "scripts/main.vela".into(),
                case["source"].as_str().expect("source").into(),
            );
            if crlf {
                for text in spec.files.values_mut() {
                    *text = text.replace('\n', "\r\n");
                }
            }
            (
                case["id"].as_str().expect("id").into(),
                case["allowed"].as_bool().expect("policy"),
                spec,
            )
        })
        .collect()
}

// Only marked references are renamed; an aliased use has its own unchanged marker.
pub(crate) fn renamed(spec: &Spec) -> Spec {
    let mut result = spec.clone();
    for text in result.files.values_mut() {
        for marker in ["declaration", "import", "use"] {
            *text = text.replace(
                &format!("[[{marker}:start]]grant[[{marker}:end]]"),
                &format!("[[{marker}:start]]award[[{marker}:end]]"),
            );
        }
    }
    result
}

pub(crate) fn local_cases(crlf: bool) -> Vec<Spec> {
    let catalog = load("rename-local-collisions");
    catalog.oracle["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .map(|case| {
            let mut spec = catalog.clone();
            spec.files.insert(
                "scripts/main.vela".into(),
                case["source"].as_str().expect("source").into(),
            );
            spec.oracle = case.clone();
            if crlf {
                for text in spec.files.values_mut() {
                    *text = text.replace('\n', "\r\n");
                }
            }
            spec
        })
        .collect()
}

pub(crate) fn renamed_local(spec: &Spec) -> Spec {
    let mut result = spec.clone();
    let name = spec.oracle["newName"].as_str().expect("new name");
    let text = result.files.get_mut("scripts/main.vela").expect("main");
    for marker in ["target", "use1", "use2"] {
        *text = text.replace(
            &format!("[[{marker}:start]]value[[{marker}:end]]"),
            &format!("[[{marker}:start]]{name}[[{marker}:end]]"),
        );
    }
    result
}
