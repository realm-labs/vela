use vela_analysis::facts::AnalysisFacts;
use vela_analysis::hints::type_fact_from_hint;
use vela_analysis::type_fact::TypeFact;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{EnumVariantFieldsHint, HirTypeHint};

use crate::{DocumentId, LanguageServiceDatabases, LineIndex, Position};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignatureHelp {
    active_signature: usize,
    active_parameter: usize,
    signatures: Vec<SignatureInformation>,
}

impl SignatureHelp {
    #[must_use]
    pub const fn active_signature(&self) -> usize {
        self.active_signature
    }

    #[must_use]
    pub const fn active_parameter(&self) -> usize {
        self.active_parameter
    }

    #[must_use]
    pub fn signatures(&self) -> &[SignatureInformation] {
        &self.signatures
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignatureInformation {
    label: String,
    parameters: Vec<SignatureParameter>,
}

impl SignatureInformation {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn parameters(&self) -> &[SignatureParameter] {
        &self.parameters
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignatureParameter {
    label: String,
}

impl SignatureParameter {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct CallContext {
    callee: String,
    active_parameter: usize,
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn signature_help(
        &self,
        document_id: &DocumentId,
        position: Position,
    ) -> Option<SignatureHelp> {
        let source = self.source_db().records().get(document_id)?;
        let context = call_context_at(source.text(), position)?;
        let signatures = self.signature_candidates(&context.callee);
        if signatures.is_empty() {
            return None;
        }
        let max_parameter = signatures[0].parameters.len().saturating_sub(1);
        Some(SignatureHelp {
            active_signature: 0,
            active_parameter: context.active_parameter.min(max_parameter),
            signatures,
        })
    }

    pub(crate) fn signature_candidates(&self, callee: &str) -> Vec<SignatureInformation> {
        let mut signatures = self.script_signatures(callee);
        signatures.extend(self.script_variant_signatures(callee));
        signatures.extend(self.schema_signatures(callee));
        signatures
    }

    fn script_signatures(&self, callee: &str) -> Vec<SignatureInformation> {
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph(graph);
        graph
            .declarations()
            .filter(|declaration| {
                declaration.kind == DeclarationKind::Function
                    && (declaration.name == callee
                        || qualified_declaration_label(graph, declaration.id) == callee)
            })
            .filter_map(|declaration| {
                let fact = facts.declaration(declaration.id)?;
                let TypeFact::Function { params, returns } = fact else {
                    return None;
                };
                let signature = graph.function_signature(declaration.id)?;
                let parameters = signature
                    .params
                    .iter()
                    .enumerate()
                    .map(|(index, param)| {
                        let type_fact = params.get(index).cloned().unwrap_or(TypeFact::Unknown);
                        let type_fact = if matches!(type_fact, TypeFact::Unknown) {
                            param
                                .type_hint
                                .as_ref()
                                .and_then(|hint| {
                                    schema_fact_for_hint(hint, self.schema_db().facts())
                                })
                                .unwrap_or(TypeFact::Unknown)
                        } else {
                            type_fact
                        };
                        SignatureParameter {
                            label: format!("{}: {}", param.name, type_fact.display_name()),
                        }
                    })
                    .collect::<Vec<_>>();
                Some(SignatureInformation {
                    label: signature_label(&declaration.name, &parameters, returns),
                    parameters,
                })
            })
            .collect()
    }

    fn script_variant_signatures(&self, callee: &str) -> Vec<SignatureInformation> {
        let graph = self.hir_db().graph();
        graph
            .declarations()
            .filter(|declaration| declaration.kind == DeclarationKind::Enum)
            .filter_map(|declaration| {
                let owner = qualified_declaration_label(graph, declaration.id);
                let shape = graph.enum_shape(declaration.id)?;
                Some((declaration, owner, shape))
            })
            .flat_map(|(declaration, owner, shape)| {
                shape.variants.iter().filter_map(move |variant| {
                    if !variant_callee_matches(
                        callee,
                        declaration.name.as_str(),
                        &owner,
                        &variant.name,
                    ) {
                        return None;
                    }
                    let EnumVariantFieldsHint::Tuple(fields) = &variant.fields else {
                        return None;
                    };
                    let parameters = fields
                        .iter()
                        .map(|field| {
                            let fact = field.type_hint.as_ref().map_or(TypeFact::Unknown, |hint| {
                                signature_type_fact(graph, hint, self.schema_db().facts())
                            });
                            SignatureParameter {
                                label: format!("{}: {}", field.name, fact.display_name()),
                            }
                        })
                        .collect::<Vec<_>>();
                    Some(SignatureInformation {
                        label: signature_label(
                            &format!("{owner}::{}", variant.name),
                            &parameters,
                            &TypeFact::enum_type(&owner, Some(&variant.name)),
                        ),
                        parameters,
                    })
                })
            })
            .collect()
    }

    fn schema_signatures(&self, callee: &str) -> Vec<SignatureInformation> {
        self.schema_db()
            .facts()
            .functions()
            .filter(|function| {
                function.name == callee
                    || function
                        .name
                        .rsplit("::")
                        .next()
                        .is_some_and(|name| name == callee)
            })
            .filter_map(|function| {
                let TypeFact::Function { params, returns } = function.fact else {
                    return None;
                };
                let parameters = params
                    .iter()
                    .enumerate()
                    .map(|(index, fact)| SignatureParameter {
                        label: format!("arg{index}: {}", fact.display_name()),
                    })
                    .collect::<Vec<_>>();
                Some(SignatureInformation {
                    label: signature_label(&function.name, &parameters, &returns),
                    parameters,
                })
            })
            .collect()
    }
}

fn call_context_at(text: &str, position: Position) -> Option<CallContext> {
    let offset = LineIndex::new(text).offset(position);
    let open = active_call_open(text, offset)?;
    let callee = callee_before_open(text, open)?;
    Some(CallContext {
        callee,
        active_parameter: active_parameter_index(&text[open + 1..offset]),
    })
}

fn active_call_open(text: &str, offset: usize) -> Option<usize> {
    let mut stack = Vec::new();
    for (index, ch) in text[..offset].char_indices() {
        match ch {
            '(' => stack.push(index),
            ')' => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.pop()
}

fn callee_before_open(text: &str, open: usize) -> Option<String> {
    let before = text[..open].trim_end();
    let end = before.len();
    let start = before
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!is_callee_continue(ch)).then_some(index + ch.len_utf8()))
        .unwrap_or(0);
    (start < end).then(|| before[start..end].to_owned())
}

fn active_parameter_index(args_text: &str) -> usize {
    let mut depth = 0usize;
    let mut active = 0usize;
    for ch in args_text.chars() {
        match ch {
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => active = active.saturating_add(1),
            _ => {}
        }
    }
    active
}

fn is_callee_continue(ch: char) -> bool {
    ch == '_' || ch == ':' || ch == '.' || ch.is_ascii_alphanumeric()
}

fn signature_label(name: &str, parameters: &[SignatureParameter], returns: &TypeFact) -> String {
    let params = parameters
        .iter()
        .map(|param| param.label.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}({params}) -> {}", returns.display_name())
}

fn variant_callee_matches(callee: &str, enum_name: &str, owner: &str, variant: &str) -> bool {
    callee == variant
        || callee == format!("{enum_name}::{variant}")
        || callee == format!("{owner}::{variant}")
}

fn signature_type_fact(
    graph: &ModuleGraph,
    hint: &HirTypeHint,
    schema: &vela_analysis::registry::RegistryFacts,
) -> TypeFact {
    let fact = type_fact_from_hint(graph, hint);
    if matches!(fact, TypeFact::Unknown) {
        schema_fact_for_hint(hint, schema).unwrap_or(TypeFact::Unknown)
    } else {
        fact
    }
}

fn schema_fact_for_hint(
    hint: &HirTypeHint,
    schema: &vela_analysis::registry::RegistryFacts,
) -> Option<TypeFact> {
    if !hint.args.is_empty() {
        return None;
    }
    let qualified = hint.path.join("::");
    schema
        .type_fact(&qualified)
        .or_else(|| hint.path.last().and_then(|name| schema.type_fact(name)))
        .cloned()
}

fn qualified_declaration_label(
    graph: &ModuleGraph,
    declaration: vela_hir::ids::HirDeclId,
) -> String {
    let Some(declaration) = graph.declaration(declaration) else {
        return String::new();
    };
    let Some(module_path) = graph.module_path(declaration.module) else {
        return declaration.name.clone();
    };
    let module = module_path.join();
    if module.is_empty() {
        declaration.name.clone()
    } else {
        format!("{module}::{}", declaration.name)
    }
}

#[cfg(test)]
mod tests {
    use vela_analysis::registry::RegistryFacts;

    use super::*;
    use crate::{
        SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
    };

    #[test]
    fn signature_help_tracks_active_parameter() {
        let document = DocumentId::from("/workspace/scripts/game/main.vela");
        let text = r#"
            pub fn grant(player: Player, amount: i64) -> bool { return true }
            pub fn main(player: Player) { grant(player, 1) }
        "#;
        let files = vec![SourceFileSnapshot::new(document.clone(), text)];
        let config = WorkspaceConfig::workspace([WorkspaceRoot::from("/workspace/scripts")]);
        let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
        let mut databases = LanguageServiceDatabases::new();
        let mut schema = RegistryFacts::default();
        schema.insert_type("Player", TypeFact::host("Player"));
        databases.set_schema_facts(schema);
        databases.update(&project);

        let main_line = text.lines().nth(2).expect("main line should exist");
        let argument_offset = main_line
            .find("1)")
            .expect("second argument should exist in main call");
        let position = Position::new(2, argument_offset);
        let help = databases
            .signature_help(&document, position)
            .expect("signature help should resolve script function");

        assert_eq!(help.active_parameter(), 1);
        assert_eq!(
            help.signatures()[0].label(),
            "grant(player: Player, amount: i64) -> bool"
        );
        assert_eq!(help.signatures()[0].parameters()[1].label(), "amount: i64");
    }
}
