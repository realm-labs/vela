use super::{FixtureWorkspace, Spec, load};
use serde_json::Value;

pub(crate) fn spec(crlf: bool) -> Spec {
    spec_for("reference-rename-coordinates", crlf)
}

pub(crate) fn named_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-named-parameters", crlf)
}

pub(crate) fn method_parameter_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-method-parameters", crlf)
}

pub(crate) fn required_parameter_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-required-parameters", crlf)
}

pub(crate) fn field_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-record-fields", crlf)
}

pub(crate) fn variant_field_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-variant-fields", crlf)
}

pub(crate) fn tuple_field_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-tuple-fields", crlf)
}

pub(crate) fn default_binding_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-default-bindings", crlf)
}

pub(crate) fn schema_field_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-schema-fields", crlf)
}

pub(crate) fn schema_method_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-schema-methods", crlf)
}

pub(crate) fn schema_function_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-schema-functions", crlf)
}

pub(crate) fn schema_import_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-schema-imports", crlf)
}

pub(crate) fn import_boundary_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-import-boundaries", crlf)
}

pub(crate) fn imported_values_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-imported-values", crlf)
}

pub(crate) fn imported_types_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-imported-types", crlf)
}

pub(crate) fn schema_type_import_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-schema-type-imports", crlf)
}

pub(crate) fn schema_capture_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-schema-capture", crlf)
}

pub(crate) fn schema_lookup_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-schema-lookup", crlf)
}

pub(crate) fn schema_variant_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-schema-variants", crlf)
}

pub(crate) fn schema_variant_import_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-schema-variant-imports", crlf)
}

pub(crate) fn schema_variant_capture_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-schema-variant-capture", crlf)
}

pub(crate) fn schema_variant_lookup_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-schema-variant-lookup", crlf)
}

pub(crate) fn schema_variant_ambiguity_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-schema-variant-ambiguity", crlf)
}

pub(crate) fn source_variant_import_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-source-variant-imports", crlf)
}

pub(crate) fn private_variant_import_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-private-variant-imports", crlf)
}

pub(crate) fn variant_ambiguity_removal_spec(crlf: bool) -> Spec {
    spec_for("reference-rename-variant-ambiguity-removal", crlf)
}

pub(crate) fn edits<'a>(spec: &'a Spec, group: &str) -> &'a [Value] {
    let definition = &spec.oracle["groups"][group];
    definition
        .get("edits")
        .unwrap_or(&definition["sites"])
        .as_array()
        .expect("edit sites")
}

pub(crate) fn replacement(site: &Value, name: &str) -> String {
    format!("{name}{}", site["suffix"].as_str().unwrap_or(""))
}

pub(crate) fn local_marker(
    spec: &Spec,
    fixture: &FixtureWorkspace,
    check: &Value,
    target: bool,
) -> super::Marker {
    let file = check["file"].as_str().expect("file");
    let key = if target { "target" } else { "marker" };
    let mut range = fixture.disk[file].markers[check[key].as_str().expect("marker")];
    let group_key = if target { "targetGroup" } else { "queryGroup" };
    if let Some(group) = check[group_key].as_str()
        && spec.oracle["groups"][group]["name"] != "value"
    {
        range.start = range.end;
        range.start.byte += 2;
        range.start.character += 2;
        range.end = range.start;
        range.end.byte += 5;
        range.end.character += 5;
    }
    assert_eq!(
        &fixture.disk[file].text[range.start.byte..range.end.byte],
        "value"
    );
    range
}

fn spec_for(id: &str, crlf: bool) -> Spec {
    let mut spec = load(id);
    if crlf {
        for text in spec.files.values_mut() {
            *text = text.replace('\n', "\r\n");
        }
    }
    spec
}

pub(crate) fn sites(spec: &Spec, query: &Value) -> Vec<Value> {
    query["group"].as_str().map_or_else(Vec::new, |group| {
        spec.oracle["groups"][group]["sites"]
            .as_array()
            .expect("reference sites")
            .clone()
    })
}

pub(crate) fn rejections(spec: &Spec, group: &str) -> Vec<Value> {
    let mut names = spec.oracle["rejections"][group]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for owner in spec.oracle["rejectionGroups"][group]
        .as_array()
        .into_iter()
        .flatten()
    {
        let name = spec.oracle["groups"][owner.as_str().expect("collision owner")]["name"]
            .as_str()
            .expect("collision owner's current spelling");
        names.push(name.into());
    }
    names
}

pub(crate) fn renamed(spec: &Spec, group: &str, new_name: &str) -> Spec {
    let mut result = spec.clone();
    let definition = &spec.oracle["groups"][group];
    let old_name = definition["name"].as_str().expect("old name");
    for site in edits(spec, group) {
        let file = site["file"].as_str().expect("file");
        let marker = site["marker"].as_str().expect("marker");
        let old = format!("[[{marker}:start]]{old_name}[[{marker}:end]]");
        let suffix = site["suffix"].as_str().unwrap_or("");
        let new = format!("[[{marker}:start]]{new_name}[[{marker}:end]]{suffix}");
        let source = result.files.get_mut(file).expect("source");
        assert_eq!(source.matches(&old).count(), 1, "independent marked edit");
        *source = source.replace(&old, &new);
    }
    result.oracle["groups"][group]["name"] = new_name.into();
    if let Some(entry) = definition["schemaEntry"].as_object() {
        let collection = entry["collection"].as_str().expect("schema collection");
        let index = entry["index"].as_u64().expect("schema entry index") as usize;
        let original = result.oracle["schema"][collection][index]["name"]
            .as_str()
            .expect("schema name");
        let renamed_schema_name = if matches!(collection, "functions" | "types" | "traits") {
            original.rsplit_once("::").map_or_else(
                || new_name.to_owned(),
                |(owner, _)| format!("{owner}::{new_name}"),
            )
        } else {
            new_name.to_owned()
        };
        result.oracle["schema"][collection][index]["name"] = renamed_schema_name.clone().into();
        if matches!(collection, "types" | "traits") {
            result.oracle["schema"][collection][index]["fact"]["name"] = renamed_schema_name.into();
        }
        if collection == "variants" {
            result.oracle["schema"][collection][index]["fact"]["variant"] = new_name.into();
        }
    }
    for site in result.oracle["groups"][group]["sites"]
        .as_array_mut()
        .expect("sites")
    {
        site.as_object_mut().expect("site").remove("suffix");
    }
    if let Some(qualified) = definition["qualified"].as_str() {
        let separator = if qualified.contains('.') { "." } else { "::" };
        let (owner, _) = qualified.rsplit_once(separator).expect("qualified owner");
        result.oracle["groups"][group]["qualified"] =
            format!("{owner}{separator}{new_name}").into();
    }
    result
}

pub(crate) fn assert_parsed(fixture: &FixtureWorkspace) {
    for (file, document) in &fixture.disk {
        if file.ends_with(".vela") {
            assert!(
                vela_syntax::parse::parse_source(&document.text)
                    .diagnostics()
                    .is_empty(),
                "{file}"
            );
            if let Some(marker) = document.markers.get("recovery-label") {
                use vela_syntax::ast::{AstNode, SyntaxCallExpr};
                let parsed = vela_syntax::parse::parse_source(&document.text);
                let arguments = parsed
                    .tree()
                    .syntax()
                    .descendants()
                    .filter_map(SyntaxCallExpr::cast)
                    .flat_map(|call| call.arguments())
                    .collect::<Vec<_>>();
                let argument = arguments
                    .iter()
                    .find(|argument| {
                        argument.name_token().is_some_and(|name| {
                            usize::from(name.text_range().start()) == marker.start.byte
                        })
                    })
                    .expect("recovered named argument");
                assert!(
                    argument.expression().is_none(),
                    "recovery fixture must retain its missing value"
                );
            }
        }
    }
}
