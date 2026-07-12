use std::collections::BTreeMap;

use vela_analysis::hints::type_fact_from_hint_in_module;
use vela_analysis::registry::RegistryFacts;
use vela_analysis::type_fact::TypeFact;
use vela_common::{PrimitiveTag, ShapeId};
use vela_def::{
    FunctionId, TypeId, script_field_id, script_function_id, script_global_id, script_type_id,
    script_type_path, script_variant_id,
};
use vela_hir::attributes::schema_id_attr;
use vela_hir::body::{HirBody, HirBodyRoot};
use vela_hir::ids::{HirDeclId, ModuleId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::{EnumVariantFieldsHint, FunctionSignature, HirTypeHint};
use vela_mir::{
    CompileFieldAccess, CompileFieldDescriptor, CompileFunctionAccess, CompileFunctionClass,
    CompileFunctionDescriptor, CompileGlobalDescriptor, CompileGuardKey, CompileGuardTarget,
    CompileMethodAccess, CompileMethodClass, CompileMethodDescriptor, CompileParameter,
    CompileParameterDefault, CompilePositionalPolicy, CompileSignature, CompileTypeClass,
    CompileTypeDescriptor, CompileVariantDescriptor, HostTypeTarget, MethodExecutableTarget,
    MirEffect, MirGuardLocation, MirSourceOrigin, MirTypeContract,
};
use vela_registry::{TypeHintDef, TypeKindDef};

use super::contracts::ContractBoundary;
use super::external::ExternalCatalog;
use super::{GenerationBuilder, SemanticRoots, input_error, registry_input_error};
use crate::compiler::error::CompileResult;

impl GenerationBuilder<'_, '_> {
    pub(super) fn insert_script_schema(&mut self) -> CompileResult<()> {
        self.index_script_types()?;
        let declarations = self
            .request
            .type_symbols
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for declaration in declarations {
            self.insert_script_type(declaration)?;
        }
        self.insert_script_globals()
    }

    fn index_script_types(&mut self) -> CompileResult<()> {
        for (declaration, symbol) in self.request.type_symbols {
            let package = self
                .request
                .graph
                .declaration(*declaration)
                .and_then(|metadata| self.request.graph.module_package(metadata.module))
                .ok_or_else(registry_input_error)?;
            let explicit =
                schema_id_attr(self.request.graph.declaration_attrs(*declaration)).map(u128::from);
            let id = script_type_id(package.as_str(), symbol, explicit);
            self.type_ids.insert(*declaration, id);
            self.type_names.insert(id, symbol.clone());
            if let Some(shape) = self.request.graph.struct_shape(*declaration) {
                let mut names = shape
                    .fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .collect::<Vec<_>>();
                names.sort_unstable();
                self.type_shapes
                    .insert(id, vela_common::script_shape_id(symbol, names.into_iter()));
            }
            if let Some(shape) = self.request.graph.enum_shape(*declaration) {
                for variant in &shape.variants {
                    self.variant_ids.insert(
                        (*declaration, variant.name.clone()),
                        script_variant_id(
                            package.as_str(),
                            symbol,
                            &variant.name,
                            schema_id_attr(&variant.attrs).map(u128::from),
                        ),
                    );
                }
            }
        }
        Ok(())
    }

    fn insert_script_type(&mut self, declaration: HirDeclId) -> CompileResult<()> {
        let metadata = self
            .request
            .graph
            .declaration(declaration)
            .ok_or_else(registry_input_error)?;
        let type_id = self.type_ids[&declaration];
        let type_name = self.type_names[&type_id].clone();
        let package = self
            .request
            .graph
            .module_package(metadata.module)
            .ok_or_else(registry_input_error)?
            .as_str();
        let origin = MirSourceOrigin::declaration(declaration, metadata.span);
        let mut type_fields = Vec::new();
        let mut type_variants = Vec::new();

        match metadata.kind {
            DeclarationKind::Struct => {
                let shape = self
                    .request
                    .graph
                    .struct_shape(declaration)
                    .ok_or_else(registry_input_error)?;
                let slots = shape
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(order, field)| {
                        let id = script_field_id(
                            package,
                            &type_name,
                            None,
                            &field.name,
                            schema_id_attr(&field.attrs).map(u128::from),
                        );
                        self.field_ids
                            .insert((declaration, None, field.name.clone()), id);
                        (field.name.clone(), id, order, field)
                    })
                    .collect::<Vec<_>>();
                type_fields.extend(slots.iter().map(|(_, id, _, _)| *id));
                for (_, id, order, field) in slots {
                    let contract = field
                        .type_hint
                        .as_ref()
                        .and_then(|hint| self.type_contract_for_hint(metadata.module, hint))
                        .and_then(meaningful_contract);
                    if let Some(contract) = &contract {
                        self.remember_contract(contract, origin);
                        if let Some(body) = field.default_body {
                            self.validate_schema_default_contract(
                                body,
                                contract,
                                &field.name,
                                field.default_value_span.unwrap_or(field.span),
                            )?;
                        }
                    }
                    self.targets
                        .insert_field_descriptor(
                            CompileFieldDescriptor {
                                id,
                                owner: type_id,
                                variant: None,
                                name: field.name.clone(),
                                contract: contract.clone(),
                                declaration_order: checked_index(order, origin, "field order")?,
                                access: CompileFieldAccess::script(),
                                host_runtime: None,
                            },
                            origin,
                        )
                        .map_err(input_error)?;
                    if let Some(contract) = contract {
                        self.insert_guard_once(
                            CompileGuardKey::Field(id),
                            CompileGuardTarget::new(
                                contract,
                                MirGuardLocation::Field,
                                field.name.clone(),
                            ),
                            origin,
                        )?;
                    }
                }
            }
            DeclarationKind::Enum => {
                let shape = self
                    .request
                    .graph
                    .enum_shape(declaration)
                    .ok_or_else(registry_input_error)?;
                for (variant_order, variant) in shape.variants.iter().enumerate() {
                    let variant_id = self.variant_ids[&(declaration, variant.name.clone())];
                    type_variants.push(variant_id);
                    let fields = enum_fields(&variant.fields);
                    let mut field_ids = Vec::new();
                    for (field_order, field) in fields.iter().enumerate() {
                        let id = script_field_id(
                            package,
                            &type_name,
                            Some(&variant.name),
                            &field.name,
                            schema_id_attr(field.attrs).map(u128::from),
                        );
                        self.field_ids.insert(
                            (declaration, Some(variant.name.clone()), field.name.clone()),
                            id,
                        );
                        field_ids.push(id);
                        let contract = field
                            .hint
                            .and_then(|hint| self.type_contract_for_hint(metadata.module, hint))
                            .and_then(meaningful_contract);
                        if let Some(contract) = &contract {
                            self.remember_contract(contract, origin);
                            if let Some(body) = field.default_body {
                                self.validate_schema_default_contract(
                                    body,
                                    contract,
                                    &field.name,
                                    field.default_value_span.unwrap_or(field.span),
                                )?;
                            }
                        }
                        self.targets
                            .insert_field_descriptor(
                                CompileFieldDescriptor {
                                    id,
                                    owner: type_id,
                                    variant: Some(variant_id),
                                    name: field.name.clone(),
                                    contract: contract.clone(),
                                    declaration_order: checked_index(
                                        field_order,
                                        origin,
                                        "variant field order",
                                    )?,
                                    access: CompileFieldAccess::script(),
                                    host_runtime: None,
                                },
                                origin,
                            )
                            .map_err(input_error)?;
                        if let Some(contract) = contract {
                            self.insert_guard_once(
                                CompileGuardKey::Field(id),
                                CompileGuardTarget::new(
                                    contract,
                                    MirGuardLocation::Field,
                                    field.name.clone(),
                                ),
                                origin,
                            )?;
                        }
                    }
                    self.targets
                        .insert_variant_descriptor(
                            CompileVariantDescriptor {
                                id: variant_id,
                                owner: type_id,
                                name: variant.name.clone(),
                                fields: field_ids,
                                declaration_order: checked_index(
                                    variant_order,
                                    origin,
                                    "variant order",
                                )?,
                            },
                            origin,
                        )
                        .map_err(input_error)?;
                }
            }
            _ => return Ok(()),
        }

        self.targets
            .insert_script_type(
                declaration,
                CompileTypeDescriptor {
                    id: type_id,
                    canonical_name: script_type_path(package, &type_name).canonical_name(),
                    runtime_name: type_name,
                    class: match metadata.kind {
                        DeclarationKind::Struct => CompileTypeClass::ScriptRecord,
                        DeclarationKind::Enum => CompileTypeClass::ScriptEnum,
                        _ => unreachable!(),
                    },
                    shape: self.type_shapes.get(&type_id).copied(),
                    fields: type_fields,
                    variants: type_variants,
                },
                origin,
            )
            .map_err(input_error)
    }

    fn insert_script_globals(&mut self) -> CompileResult<()> {
        let globals = self
            .request
            .global_symbols
            .iter()
            .map(|(declaration, symbol)| (*declaration, symbol.clone()))
            .collect::<Vec<_>>();
        for (declaration, symbol) in globals {
            let metadata = self
                .request
                .graph
                .declaration(declaration)
                .ok_or_else(registry_input_error)?;
            let global = self
                .request
                .graph
                .global_metadata(declaration)
                .ok_or_else(registry_input_error)?;
            let Some(contract) = self.type_contract_for_hint(metadata.module, &global.type_hint)
            else {
                continue;
            };
            let package = self
                .request
                .graph
                .module_package(metadata.module)
                .ok_or_else(registry_input_error)?;
            let id = script_global_id(package.as_str(), &symbol);
            let origin = MirSourceOrigin::declaration(declaration, metadata.span);
            self.remember_contract(&contract, origin);
            self.targets
                .insert_global(
                    declaration,
                    CompileGlobalDescriptor {
                        id,
                        name: symbol.clone(),
                        contract: contract.clone(),
                    },
                    origin,
                )
                .map_err(input_error)?;
            if !matches!(contract, MirTypeContract::Any) {
                self.insert_guard_once(
                    CompileGuardKey::Global(declaration),
                    CompileGuardTarget::new(
                        contract,
                        MirGuardLocation::Global,
                        metadata.name.clone(),
                    ),
                    origin,
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn insert_script_callables(&mut self) -> CompileResult<()> {
        self.insert_script_functions()?;
        self.insert_script_methods()
    }

    fn insert_script_functions(&mut self) -> CompileResult<()> {
        let functions = self
            .request
            .script_function_symbols
            .iter()
            .map(|(declaration, symbol)| (*declaration, symbol.clone()))
            .collect::<Vec<_>>();
        for (declaration, symbol) in functions {
            let metadata = self
                .request
                .graph
                .declaration(declaration)
                .ok_or_else(registry_input_error)?;
            let body = self
                .request
                .graph
                .function_body(declaration)
                .ok_or_else(registry_input_error)?;
            let signature = self
                .request
                .graph
                .function_signature(declaration)
                .ok_or_else(registry_input_error)?;
            let package = self
                .request
                .graph
                .module_package(metadata.module)
                .ok_or_else(registry_input_error)?;
            let function = script_function_id(package.as_str(), &symbol);
            self.function_ids.insert(declaration, function);
            let origin = MirSourceOrigin::body(body.id, body.origin.span);
            self.function_code_symbols.insert(function, symbol.clone());
            let signature = self.script_signature(function, body, signature, metadata.module)?;
            self.remember_signature_contracts(&signature, origin);
            let descriptor = CompileFunctionDescriptor {
                id: function,
                class: CompileFunctionClass::Script,
                canonical_symbol: symbol.clone(),
                debug_name: metadata.name.clone(),
                signature,
                access: CompileFunctionAccess::script(
                    metadata.visibility == vela_hir::module_graph::Visibility::Public,
                ),
            };
            let selected = matches!(self.request.roots, SemanticRoots::Program)
                || self.request.roots == SemanticRoots::Function(declaration);
            if selected {
                self.targets
                    .insert_script_function(declaration, body.id, descriptor, origin)
                    .map_err(input_error)?;
            } else {
                self.targets
                    .insert_script_function_descriptor(declaration, descriptor, origin)
                    .map_err(input_error)?;
            }
        }
        Ok(())
    }

    fn insert_script_methods(&mut self) -> CompileResult<()> {
        let methods = self
            .request
            .script_methods
            .methods()
            .cloned()
            .collect::<Vec<_>>();
        for method in &methods {
            let target_type = method.owner().target_type();
            let symbol = method.symbol_seed();
            let body = self
                .request
                .graph
                .body(method.body())
                .ok_or_else(registry_input_error)?;
            let origin = MirSourceOrigin::body(body.id, body.origin.span);
            let owner = self
                .type_ids
                .iter()
                .find_map(|(declaration, id)| {
                    self.request
                        .type_symbols
                        .get(declaration)
                        .is_some_and(|symbol| symbol == target_type)
                        .then_some(*id)
                })
                .or_else(|| {
                    self.registry_facts
                        .type_target_fact(target_type)
                        .map(|target| target.semantic)
                })
                .map_or_else(|| self.ensure_opaque_external_type(target_type, origin), Ok)?;
            let function = script_function_id(method.owner().package().as_str(), &symbol);
            self.function_code_symbols.insert(function, symbol.clone());
            let target = MethodExecutableTarget {
                method: method.method_id(),
                function,
                owner,
                node: method.node(),
            };
            self.method_targets.insert((method.node(), owner), target);
            if !self.type_names.contains_key(&owner) {
                self.ensure_external_type(owner, origin)?;
            }
            let full_signature = self.script_signature(
                function,
                body,
                method.signature(),
                method.signature_module(),
            )?;
            self.remember_signature_contracts(&full_signature, origin);
            self.targets
                .insert_function_descriptor(
                    CompileFunctionDescriptor {
                        id: function,
                        class: CompileFunctionClass::Script,
                        canonical_symbol: symbol.clone(),
                        debug_name: format!("{target_type}::{}", method.name()),
                        signature: full_signature.clone(),
                        access: CompileFunctionAccess::script(false),
                    },
                    origin,
                )
                .map_err(input_error)?;
            self.targets
                .insert_method_descriptor(
                    CompileMethodDescriptor {
                        id: method.method_id(),
                        owner,
                        member_name: method.name().to_owned(),
                        debug_name: format!("{target_type}::{}", method.name()),
                        class: CompileMethodClass::Script {
                            executable: target,
                            owner_name: target_type.to_owned(),
                            code_symbol: symbol,
                        },
                        signature: CompileSignature {
                            parameters: full_signature.parameters.iter().skip(1).cloned().collect(),
                            ..full_signature
                        },
                        access: CompileMethodAccess::script(),
                    },
                    origin,
                )
                .map_err(input_error)?;
            if matches!(self.request.roots, SemanticRoots::Program) {
                self.targets
                    .insert_script_method(body.id, target, origin)
                    .map_err(input_error)?;
            } else {
                self.targets
                    .insert_script_method_target(target, origin)
                    .map_err(input_error)?;
            }
        }
        Ok(())
    }

    fn script_signature(
        &mut self,
        function: FunctionId,
        body: &HirBody,
        signature: &FunctionSignature,
        module: ModuleId,
    ) -> CompileResult<CompileSignature> {
        let mut parameters = Vec::new();
        for (index, (parameter, hint)) in body.params.iter().zip(&signature.params).enumerate() {
            let contract = hint
                .type_hint
                .as_ref()
                .and_then(|hint| self.type_contract_for_hint(module, hint))
                .and_then(meaningful_contract);
            let origin = MirSourceOrigin::body(body.id, parameter.origin.span);
            parameters.push(CompileParameter {
                name: hint.name.clone(),
                contract: contract.clone(),
                default: parameter.default_body.map_or(
                    CompileParameterDefault::Required,
                    CompileParameterDefault::HirBody,
                ),
                origin: Some(origin),
            });
            if let Some(contract) = contract.clone() {
                let parameter_index = checked_index(index, origin, "parameter index")?;
                self.insert_guard_once(
                    CompileGuardKey::Parameter {
                        function,
                        parameter: parameter_index,
                    },
                    CompileGuardTarget::new(
                        contract.clone(),
                        MirGuardLocation::Parameter {
                            index: parameter_index,
                        },
                        hint.name.clone(),
                    ),
                    origin,
                )?;
                if let Some(default_body) = parameter.default_body
                    && let Some(expression) = body_root_expression(self.request.graph, default_body)
                {
                    self.boundaries.push(ContractBoundary::function_parameter(
                        function,
                        expression,
                        contract,
                        hint.name.clone(),
                        parameter_index,
                    ));
                }
            }
        }
        let return_contract = signature
            .return_type
            .as_ref()
            .and_then(|hint| self.type_contract_for_hint(module, hint))
            .and_then(meaningful_contract);
        let origin = MirSourceOrigin::body(body.id, body.origin.span);
        if self
            .function_return_contracts
            .insert(function, return_contract.clone())
            .is_some()
        {
            return Err(input_error(vela_mir::MirBuildError::InconsistentInput {
                origin,
                message: format!(
                    "duplicate owning return contract for function #{}",
                    function.get()
                ),
            }));
        }
        if let Some(contract) = return_contract.clone() {
            self.insert_guard_once(
                CompileGuardKey::Return(function),
                CompileGuardTarget::new(contract, MirGuardLocation::Return, "return"),
                origin,
            )?;
        }
        Ok(CompileSignature {
            parameters,
            positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
            return_contract,
            effect: MirEffect::PURE,
        })
    }

    pub(super) fn insert_guard_once(
        &mut self,
        key: CompileGuardKey,
        guard: CompileGuardTarget,
        origin: MirSourceOrigin,
    ) -> CompileResult<()> {
        self.targets
            .insert_guard(key, guard, origin)
            .map_err(input_error)
    }
}

pub(super) fn hir_hint_contract(
    graph: &ModuleGraph,
    module: ModuleId,
    hint: &HirTypeHint,
    registry: &RegistryFacts,
    script_types: &BTreeMap<HirDeclId, TypeId>,
    script_shapes: &BTreeMap<TypeId, ShapeId>,
) -> Option<MirTypeContract> {
    fn fact_with_registry(
        graph: &ModuleGraph,
        module: ModuleId,
        hint: &HirTypeHint,
        registry: &RegistryFacts,
    ) -> TypeFact {
        let nested = |hint: &HirTypeHint| fact_with_registry(graph, module, hint, registry);
        let fact = match (hint.path.as_slice(), hint.args.as_slice()) {
            ([name], [value]) if name == "Array" => TypeFact::array(nested(value)),
            ([name], [key, value]) if name == "Map" => TypeFact::map(nested(key), nested(value)),
            ([name], [value]) if name == "Set" => TypeFact::set(nested(value)),
            ([name], [value]) if name == "Iterator" => TypeFact::iterator(nested(value)),
            ([name], [value]) if name == "Option" => TypeFact::option(nested(value)),
            ([name], [ok, err]) if name == "Result" => TypeFact::result(nested(ok), nested(err)),
            ([name], values) if name == HirTypeHint::UNIT_PATH && values.len() >= 2 => {
                TypeFact::tuple(values.iter().map(nested))
            }
            _ => type_fact_from_hint_in_module(graph, module, hint),
        };
        if !matches!(fact, TypeFact::Unknown) {
            return fact;
        }
        let qualified = hint.path.join("::");
        registry
            .type_fact(&qualified)
            .or_else(|| hint.path.last().and_then(|name| registry.type_fact(name)))
            .cloned()
            .unwrap_or(TypeFact::Unknown)
    }

    let fact = fact_with_registry(graph, module, hint, registry);
    contract_from_fact(&fact, registry, graph, script_types, script_shapes)
}

pub(super) fn registry_hint_contract(
    hint: &TypeHintDef,
    catalog: &ExternalCatalog,
) -> Option<MirTypeContract> {
    let fact = registry_hint_fact(hint, catalog);
    external_contract_from_fact(&fact, catalog)
}

pub(super) fn registry_hint_fact(hint: &TypeHintDef, catalog: &ExternalCatalog) -> TypeFact {
    let name = hint.path.join("::");
    match (name.as_str(), hint.args.as_slice()) {
        ("()", []) => TypeFact::UNIT,
        ("()", elements) if elements.len() >= 2 => TypeFact::tuple(
            elements
                .iter()
                .map(|hint| registry_hint_fact(hint, catalog)),
        ),
        ("Any", []) => TypeFact::Any,
        ("String", []) => TypeFact::STRING,
        ("Bytes", []) => TypeFact::BYTES,
        ("Array", []) => TypeFact::array(TypeFact::Unknown),
        ("Array", [value]) => TypeFact::array(registry_hint_fact(value, catalog)),
        ("Map", []) => TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
        ("Map", [key, value]) => TypeFact::map(
            registry_hint_fact(key, catalog),
            registry_hint_fact(value, catalog),
        ),
        ("Set", []) => TypeFact::set(TypeFact::Unknown),
        ("Set", [value]) => TypeFact::set(registry_hint_fact(value, catalog)),
        ("Iterator", []) => TypeFact::iterator(TypeFact::Unknown),
        ("Iterator", [value]) => TypeFact::iterator(registry_hint_fact(value, catalog)),
        ("Option", []) => TypeFact::option(TypeFact::Unknown),
        ("Option", [value]) => TypeFact::option(registry_hint_fact(value, catalog)),
        ("Result", []) => TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
        ("Result", [ok, err]) => TypeFact::result(
            registry_hint_fact(ok, catalog),
            registry_hint_fact(err, catalog),
        ),
        ("Range", []) => TypeFact::Range,
        ("Function", []) => TypeFact::function(Vec::new(), TypeFact::Unknown),
        ("Closure", []) => TypeFact::Closure,
        (name, []) => PrimitiveTag::from_name(name)
            .map(TypeFact::primitive)
            .or_else(|| catalog.type_by_source(name).map(registered_type_fact))
            .unwrap_or(TypeFact::Unknown),
        _ => TypeFact::Unknown,
    }
}

pub(super) fn contract_from_fact(
    fact: &TypeFact,
    registry: &RegistryFacts,
    graph: &ModuleGraph,
    script_types: &BTreeMap<HirDeclId, TypeId>,
    script_shapes: &BTreeMap<TypeId, ShapeId>,
) -> Option<MirTypeContract> {
    structural_contract(fact, |name, host| {
        if let Some((_, type_id)) = script_types.iter().find(|(declaration, _)| {
            graph.qualified_declaration_name(**declaration).as_deref() == Some(name)
                || graph
                    .declaration(**declaration)
                    .is_some_and(|declaration| declaration.name == name)
        }) {
            return Some(script_shapes.get(type_id).map_or(
                MirTypeContract::Definition(*type_id),
                |shape| MirTypeContract::Shape {
                    type_id: *type_id,
                    shape: *shape,
                },
            ));
        }
        let target = registry.type_target_fact(name)?;
        if host {
            Some(MirTypeContract::Host(HostTypeTarget {
                semantic: target.semantic,
                runtime: target.host_runtime?,
            }))
        } else {
            Some(MirTypeContract::Definition(target.semantic))
        }
    })
}

fn external_contract_from_fact(
    fact: &TypeFact,
    catalog: &ExternalCatalog,
) -> Option<MirTypeContract> {
    structural_contract(fact, |name, host| {
        let ty = catalog.type_by_source(name)?;
        if host {
            Some(MirTypeContract::Host(HostTypeTarget {
                semantic: ty.id,
                runtime: vela_common::HostTypeId::new(u64::try_from(ty.host_runtime_id?).ok()?),
            }))
        } else {
            Some(MirTypeContract::Definition(ty.id))
        }
    })
}

fn structural_contract(
    fact: &TypeFact,
    mut definition: impl FnMut(&str, bool) -> Option<MirTypeContract> + Copy,
) -> Option<MirTypeContract> {
    Some(match fact {
        TypeFact::Unknown | TypeFact::Any | TypeFact::Union(_) => MirTypeContract::Any,
        TypeFact::Never => MirTypeContract::Any,
        TypeFact::Primitive(tag) => MirTypeContract::Primitive(*tag),
        TypeFact::Range => MirTypeContract::Range,
        TypeFact::Array { element } => MirTypeContract::Array(contract_box(element, definition)),
        TypeFact::Map { key, value } => MirTypeContract::Map {
            key: contract_box(key, definition),
            value: contract_box(value, definition),
        },
        TypeFact::Set { element } => MirTypeContract::Set(contract_box(element, definition)),
        TypeFact::Iterator { item } => MirTypeContract::Iterator(contract_box(item, definition)),
        TypeFact::Tuple { elements } => MirTypeContract::Tuple(
            elements
                .iter()
                .map(|fact| structural_contract(fact, definition))
                .collect(),
        ),
        TypeFact::Option { some } | TypeFact::OptionSome { some } => {
            MirTypeContract::Option(contract_box(some, definition))
        }
        TypeFact::OptionNone => MirTypeContract::Option(None),
        TypeFact::Result { ok, err } => MirTypeContract::Result {
            ok: contract_box(ok, definition),
            err: contract_box(err, definition),
        },
        TypeFact::ResultOk { ok } => MirTypeContract::Result {
            ok: contract_box(ok, definition),
            err: None,
        },
        TypeFact::ResultErr { err } => MirTypeContract::Result {
            ok: None,
            err: contract_box(err, definition),
        },
        TypeFact::Function { params, returns } => MirTypeContract::Callable {
            accepted_kinds: vela_mir::MirCallableKindSet::FUNCTION,
            positional_arity: if params.is_empty()
                && matches!(returns.as_ref(), TypeFact::Unknown | TypeFact::Any)
            {
                None
            } else {
                u32::try_from(params.len()).ok()
            },
        },
        TypeFact::Closure => MirTypeContract::Callable {
            accepted_kinds: vela_mir::MirCallableKindSet::CLOSURE,
            positional_arity: None,
        },
        TypeFact::LogicalRecord(record) => MirTypeContract::Shape {
            type_id: record.type_id(),
            shape: record.shape(),
        },
        TypeFact::Record { name } | TypeFact::Enum { name, .. } => definition(name, false)?,
        TypeFact::Host { name } => definition(name, true)?,
        TypeFact::Trait { .. } | TypeFact::Module { .. } => MirTypeContract::Any,
    })
}

fn contract_box(
    fact: &TypeFact,
    definition: impl FnMut(&str, bool) -> Option<MirTypeContract> + Copy,
) -> Option<Box<MirTypeContract>> {
    if matches!(fact, TypeFact::Unknown | TypeFact::Any) {
        None
    } else {
        structural_contract(fact, definition).map(Box::new)
    }
}

fn registered_type_fact(definition: &vela_registry::TypeDef) -> TypeFact {
    if let Some(primitive) = definition.primitive {
        return TypeFact::primitive(primitive);
    }
    let name = super::external::source_name(&definition.path);
    match definition.kind {
        TypeKindDef::Unit => TypeFact::UNIT,
        TypeKindDef::Bool => TypeFact::BOOL,
        TypeKindDef::I8 => TypeFact::I8,
        TypeKindDef::I16 => TypeFact::I16,
        TypeKindDef::I32 => TypeFact::I32,
        TypeKindDef::I64 => TypeFact::I64,
        TypeKindDef::U8 => TypeFact::U8,
        TypeKindDef::U16 => TypeFact::U16,
        TypeKindDef::U32 => TypeFact::U32,
        TypeKindDef::U64 => TypeFact::U64,
        TypeKindDef::F32 => TypeFact::F32,
        TypeKindDef::F64 => TypeFact::F64,
        TypeKindDef::Char => TypeFact::CHAR,
        TypeKindDef::String => TypeFact::STRING,
        TypeKindDef::Bytes => TypeFact::BYTES,
        TypeKindDef::Array => TypeFact::array(TypeFact::Any),
        TypeKindDef::Map => TypeFact::map(TypeFact::Any, TypeFact::Any),
        TypeKindDef::Set => TypeFact::set(TypeFact::Any),
        TypeKindDef::Iterator => TypeFact::iterator(TypeFact::Any),
        TypeKindDef::Range => TypeFact::Range,
        TypeKindDef::Function => TypeFact::function(Vec::new(), TypeFact::Any),
        TypeKindDef::Closure => TypeFact::Closure,
        TypeKindDef::Host => TypeFact::host(name),
        TypeKindDef::ScriptStruct => TypeFact::record(name),
        TypeKindDef::ScriptEnum => TypeFact::enum_type(name, None::<String>),
    }
}

fn body_root_expression(
    graph: &ModuleGraph,
    body: vela_hir::ids::HirBodyId,
) -> Option<vela_hir::ids::HirExprId> {
    match graph.body(body)?.root {
        HirBodyRoot::Expr(expression) => Some(expression),
        HirBodyRoot::Block(_) | HirBodyRoot::Empty => None,
    }
}

fn checked_index(value: usize, origin: MirSourceOrigin, description: &str) -> CompileResult<u32> {
    u32::try_from(value).map_err(|_| {
        input_error(vela_mir::MirBuildError::InconsistentInput {
            origin,
            message: format!("{description} exceeds u32::MAX"),
        })
    })
}

pub(super) fn meaningful_contract(contract: MirTypeContract) -> Option<MirTypeContract> {
    (!matches!(contract, MirTypeContract::Any)).then_some(contract)
}

struct EnumField<'a> {
    name: String,
    attrs: &'a [vela_hir::attributes::HirAttribute],
    hint: Option<&'a HirTypeHint>,
    default_body: Option<vela_hir::ids::HirBodyId>,
    default_value_span: Option<vela_common::Span>,
    span: vela_common::Span,
}

fn enum_fields(fields: &EnumVariantFieldsHint) -> Vec<EnumField<'_>> {
    match fields {
        EnumVariantFieldsHint::Unit => Vec::new(),
        EnumVariantFieldsHint::Tuple(fields) => fields
            .iter()
            .enumerate()
            .map(|(index, field)| EnumField {
                name: index.to_string(),
                attrs: &[],
                hint: field.type_hint.as_ref(),
                default_body: field.default_body,
                default_value_span: field.default_value_span,
                span: field.span,
            })
            .collect(),
        EnumVariantFieldsHint::Record(fields) => fields
            .iter()
            .map(|field| EnumField {
                name: field.name.clone(),
                attrs: &field.attrs,
                hint: field.type_hint.as_ref(),
                default_body: field.default_body,
                default_value_span: field.default_value_span,
                span: field.span,
            })
            .collect(),
    }
}
