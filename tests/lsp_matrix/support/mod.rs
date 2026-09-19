//! Test-only fixture parsing and edit oracles, independent of production text
//! indexing and language-service providers. Included only under cfg(test).
use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

mod schema;
pub(crate) use schema::{lifecycle_facts, schema_artifact, schema_lifecycle_source};
mod source;
pub(crate) use source::source_lifecycle_spec;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Point {
    pub byte: usize,
    pub line: usize,
    pub character: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Marker {
    pub start: Point,
    pub end: Point,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Document {
    pub text: String,
    pub markers: BTreeMap<String, Marker>,
}

pub(crate) fn parse_markers(source: &str) -> Result<Document, String> {
    let mut document = Document {
        text: String::new(),
        markers: BTreeMap::new(),
    };
    let mut starts = BTreeMap::new();
    let mut stack = Vec::new();
    let mut point = Point::default();
    let mut rest = source;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("[[") {
            let (token, tail) = after.split_once("]]").ok_or("unclosed marker")?;
            let (name, kind) = token
                .split_once(':')
                .map_or((token, None), |(name, kind)| (name, Some(kind)));
            if !valid_name(name) || !matches!(kind, None | Some("start" | "end")) {
                return Err(format!("invalid marker {token}"));
            }
            if document.text.ends_with('\r') && tail.starts_with('\n') {
                return Err("marker splits CRLF".to_owned());
            }
            match kind {
                Some("end") => {
                    if stack.pop() != Some(name) {
                        return Err(format!("unpaired or crossing marker {name}"));
                    }
                    let start = starts.remove(name).ok_or("missing start marker")?;
                    document
                        .markers
                        .insert(name.to_owned(), Marker { start, end: point });
                }
                _ => {
                    if document.markers.contains_key(name) || starts.contains_key(name) {
                        return Err(format!("duplicate marker {name}"));
                    }
                    if kind == Some("start") {
                        starts.insert(name, point);
                        stack.push(name);
                    } else {
                        document.markers.insert(
                            name.to_owned(),
                            Marker {
                                start: point,
                                end: point,
                            },
                        );
                    }
                }
            }
            rest = tail;
        } else {
            if rest.starts_with("]]") {
                return Err("unexpected marker close".to_owned());
            }
            let ch = rest.chars().next().ok_or("missing character")?;
            rest = &rest[ch.len_utf8()..];
            document.text.push(ch);
            point.byte += ch.len_utf8();
            if ch == '\n' {
                point.line += 1;
                point.character = 0;
            } else {
                point.character += ch.len_utf16();
            }
        }
    }
    if !stack.is_empty() {
        return Err("unclosed range marker".to_owned());
    }
    Ok(document)
}

fn valid_name(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && name
            .bytes()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == b'-')
}

pub(crate) fn safe_file(file: &str) -> Result<&str, String> {
    if file.contains(['\\', ':', '\0'])
        || file.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(format!("invalid fixture path {file}"));
    }
    Ok(file)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Action {
    pub op: String,
    pub file: String,
    pub source: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Spec {
    pub version: u32,
    pub id: String,
    pub files: BTreeMap<String, String>,
    pub actions: Vec<Action>,
    pub oracle: serde_json::Value,
}

#[derive(Debug)]
pub(crate) struct FixtureWorkspace {
    pub disk: BTreeMap<String, Document>,
    pub open: BTreeMap<String, Document>,
}

impl FixtureWorkspace {
    pub fn new(spec: &Spec) -> Result<Self, String> {
        if spec.version != 1 || spec.id.is_empty() || spec.files.is_empty() {
            return Err("invalid fixture workspace".to_owned());
        }
        let mut disk = BTreeMap::new();
        for (file, source) in &spec.files {
            disk.insert(safe_file(file)?.to_owned(), parse_markers(source)?);
        }
        Ok(Self {
            disk,
            open: BTreeMap::new(),
        })
    }

    pub fn document(&self, file: &str) -> Option<&Document> {
        self.open.get(file).or_else(|| self.disk.get(file))
    }

    pub fn apply(&mut self, action: &Action) -> Result<(), String> {
        let file = safe_file(&action.file)?;
        match action.op.as_str() {
            "open" => {
                if self.open.contains_key(file) {
                    return Err("open requires a closed disk file".to_owned());
                }
                let document = self
                    .disk
                    .get(file)
                    .ok_or("open requires a disk file")?
                    .clone();
                self.open.insert(file.to_owned(), document);
            }
            "change" => {
                if !self.open.contains_key(file) {
                    return Err("change requires an open file".to_owned());
                }
                self.open.insert(
                    file.to_owned(),
                    parse_markers(action.source.as_deref().ok_or("missing source")?)?,
                );
            }
            "save" => {
                let document = self
                    .open
                    .get(file)
                    .ok_or("save requires an open file")?
                    .clone();
                self.disk.insert(file.to_owned(), document);
            }
            "close" => {
                self.open
                    .remove(file)
                    .ok_or("close requires an open file")?;
            }
            "write" => {
                self.disk.insert(
                    file.to_owned(),
                    parse_markers(action.source.as_deref().ok_or("missing source")?)?,
                );
            }
            "delete" => {
                self.disk
                    .remove(file)
                    .ok_or("delete requires a disk file")?;
            }
            _ => return Err(format!("unknown fixture action {}", action.op)),
        }
        Ok(())
    }

    pub fn materialize(&self, root: &Path) -> std::io::Result<()> {
        // Atomic creation rejects existing roots and avoids overwriting a user's
        // files. All children were validated before reaching this method.
        std::fs::create_dir(root)?;
        for (file, document) in &self.disk {
            let target = root.join(file);
            std::fs::create_dir_all(target.parent().expect("fixture file has a parent"))?;
            std::fs::write(target, &document.text)?;
        }
        Ok(())
    }
}

pub(crate) fn load(id: &str) -> Spec {
    assert!(valid_name(id), "fixture id must be a simple name");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/lsp_matrix/fixtures");
    serde_json::from_str(
        &std::fs::read_to_string(root.join(format!("{id}.json"))).expect("fixture exists"),
    )
    .expect("fixture spec must be valid")
}

// Frozen, independently enumerated expression sets augment the older parameter
// fixtures. Every returned label is asserted, including non-parameter extras.
pub(crate) fn expected_expression_labels<'a>(
    oracle: &'a serde_json::Value,
    query: &'a serde_json::Value,
) -> Vec<&'a str> {
    query["expressions"].as_str().map_or_else(Vec::new, |set| {
        oracle["expressionSets"][set]
            .as_array()
            .expect("expression set")
            .iter()
            .chain(query["expressionAliases"].as_array().into_iter().flatten())
            .map(|label| label.as_str().expect("expression label"))
            .collect()
    })
}

pub(crate) fn offset_at(text: &str, line: usize, character: usize) -> Result<usize, String> {
    let mut start = 0;
    for (index, row) in text.split('\n').enumerate() {
        if index == line {
            let content = row.strip_suffix('\r').unwrap_or(row);
            let mut utf16 = 0;
            for (byte, ch) in content.char_indices() {
                if utf16 == character {
                    return Ok(start + byte);
                }
                utf16 += ch.len_utf16();
                if utf16 > character {
                    return Err("position splits surrogate pair".to_owned());
                }
            }
            if utf16 == character {
                return Ok(start + content.len());
            }
            return Err("character out of bounds".to_owned());
        }
        start += row.len() + 1;
    }
    Err("line out of bounds".to_owned())
}

pub(crate) struct Edit<'a> {
    pub start: (usize, usize),
    pub end: (usize, usize),
    pub text: &'a str,
}

pub(crate) fn apply_edits(text: &str, edits: &[Edit<'_>]) -> Result<String, String> {
    let mut ordered = Vec::new();
    for edit in edits {
        let start = offset_at(text, edit.start.0, edit.start.1)?;
        let end = offset_at(text, edit.end.0, edit.end.1)?;
        if end < start {
            return Err("reversed range".to_owned());
        }
        ordered.push((start, end, edit.text));
    }
    ordered.sort_by_key(|&(start, end, _)| (start, end));
    for pair in ordered.windows(2) {
        if pair[1].0 < pair[0].1 || pair[1].0 == pair[0].0 {
            return Err("overlapping edits".to_owned());
        }
    }
    let mut result = text.to_owned();
    for (start, end, replacement) in ordered.into_iter().rev() {
        result.replace_range(start..end, replacement);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
