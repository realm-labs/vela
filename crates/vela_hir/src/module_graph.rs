use std::collections::{BTreeMap, BTreeSet};

mod body_binding;
mod model;
mod names;
mod queries;
mod schema_diagnostics;
mod syntax_metadata;
mod syntax_summary;
mod validation;

use vela_common::{Diagnostic, SourceId, Span};
use vela_package::{ModuleKey, ModulePath, PackageAlias, PackageId};
use vela_syntax::ast::SyntaxSourceFile;
use vela_syntax::parse::parse_source_with_id;
use vela_syntax::{Parse as SyntaxParse, SyntaxKind};

pub use model::{
    Declaration, DeclarationIndex, DeclarationKind, Import, ImportResolution, ModuleSource,
    ResolvedImport, Visibility,
};
use names::{closest_name, import_binding_name};

use self::body_binding::{FunctionBodySource, SchemaFieldDefaultBodySource};
use crate::attributes::HirAttribute;
use crate::binding::BindingMap;
use crate::body::HirBody;
use crate::ids::{HirBodyId, HirDeclId, HirNodeId, ModuleId};
#[cfg(test)]
use crate::type_hint::HirTypeHint;
use crate::type_hint::{
    ConstMetadata, EnumShape, FunctionSignature, GlobalMetadata, ImplMetadata, StructShape,
    TraitShape,
};

use self::syntax_summary::SyntaxModuleSummary;

#[derive(Clone, Debug, Eq, PartialEq)]
struct HirModule {
    id: ModuleId,
    key: ModuleKey,
    source: SourceId,
    source_hash: Option<u64>,
    declarations: DeclarationIndex,
    imports: Vec<Import>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleGraph {
    modules: Vec<HirModule>,
    module_by_key: BTreeMap<ModuleKey, ModuleId>,
    module_children: BTreeMap<ModuleKey, BTreeSet<String>>,
    package_dependencies: BTreeMap<PackageId, BTreeMap<PackageAlias, PackageId>>,
    declarations: BTreeMap<HirDeclId, Declaration>,
    declarations_by_name: BTreeMap<String, BTreeSet<HirDeclId>>,
    declarations_by_kind: BTreeMap<DeclarationKind, BTreeSet<HirDeclId>>,
    declaration_attrs: BTreeMap<HirDeclId, Vec<HirAttribute>>,
    const_metadata: BTreeMap<HirDeclId, ConstMetadata>,
    global_metadata: BTreeMap<HirDeclId, GlobalMetadata>,
    bodies: BTreeMap<HirBodyId, HirBody>,
    body_ids_by_source: BTreeMap<SourceId, BTreeSet<HirBodyId>>,
    const_initializer_bodies: BTreeMap<HirDeclId, HirBodyId>,
    function_bodies: BTreeMap<HirDeclId, HirBodyId>,
    trait_default_method_bodies: BTreeMap<HirNodeId, HirBodyId>,
    impl_method_bodies: BTreeMap<HirNodeId, HirBodyId>,
    bindings: BTreeMap<HirDeclId, BindingMap>,
    const_initializer_bindings: BTreeMap<HirDeclId, BindingMap>,
    schema_field_default_bindings: BTreeMap<HirBodyId, BindingMap>,
    function_signatures: BTreeMap<HirDeclId, FunctionSignature>,
    struct_shapes: BTreeMap<HirDeclId, StructShape>,
    enum_shapes: BTreeMap<HirDeclId, EnumShape>,
    trait_shapes: BTreeMap<HirDeclId, TraitShape>,
    impl_metadata: BTreeMap<HirDeclId, ImplMetadata>,
    trait_default_method_bindings: BTreeMap<HirNodeId, BindingMap>,
    impl_method_bindings: BTreeMap<HirNodeId, BindingMap>,
    diagnostics: Vec<Diagnostic>,
    schema_references_validated: bool,
    next_node_id: u32,
    next_decl_id: u32,
    next_body_id: u32,
    next_block_id: u32,
    next_match_arm_id: u32,
    next_path_id: u32,
    next_scope_id: u32,
    next_stmt_id: u32,
    next_expr_id: u32,
    next_pattern_id: u32,
    next_local_id: u32,
    next_param_id: u32,
    next_capture_id: u32,
}

impl ModuleGraph {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_package_dependencies(
        package_dependencies: BTreeMap<PackageId, BTreeMap<PackageAlias, PackageId>>,
    ) -> Self {
        Self {
            package_dependencies,
            ..Self::default()
        }
    }

    pub fn add_source(&mut self, source: ModuleSource) -> ModuleId {
        let parsed = parse_source_with_id(source.id, &source.text);
        self.add_parsed_source(source, &parsed)
    }

    pub fn add_parsed_source(
        &mut self,
        source: ModuleSource,
        parsed: &SyntaxParse<SyntaxSourceFile>,
    ) -> ModuleId {
        let diagnostics = parsed.diagnostics().to_vec();
        let syntax_summary = SyntaxModuleSummary::from_parse(source.id, parsed);
        let source_hash = stable_source_hash(&source.text);
        self.add_syntax_source(
            source.id,
            source.package,
            source.path,
            diagnostics,
            Some(source_hash),
            syntax_summary,
        )
    }

    fn add_syntax_source(
        &mut self,
        source: SourceId,
        package: PackageId,
        path: ModulePath,
        diagnostics: Vec<Diagnostic>,
        source_hash: Option<u64>,
        syntax_summary: SyntaxModuleSummary,
    ) -> ModuleId {
        let module = self.next_module_id();
        let module_span = syntax_summary.module_span();

        let key = ModuleKey::new(package, path);
        if let Some(existing) = self.module_by_key.get(&key).copied() {
            self.diagnostics.push(
                Diagnostic::error(format!("duplicate module `{}`", key.path.join()))
                    .with_code("hir::duplicate_module")
                    .with_label(
                        module_span,
                        format!("module `{}` is declared more than once", key.path.join()),
                    ),
            );
            self.diagnostics.extend(diagnostics);
            return existing;
        }
        self.module_by_key.insert(key.clone(), module);
        self.index_module_path(&key);
        self.diagnostics.extend(diagnostics);

        let mut hir_module = HirModule {
            id: module,
            key,
            source,
            source_hash,
            declarations: DeclarationIndex::default(),
            imports: Vec::new(),
        };

        let mut const_initializers = Vec::new();
        let mut schema_field_defaults = Vec::new();
        let mut function_declarations = Vec::new();
        let mut trait_default_method_declarations = Vec::new();
        let mut impl_method_declarations = Vec::new();

        for (item_index, item_kind) in syntax_summary.items() {
            match item_kind {
                SyntaxKind::UseItem => {
                    let Some(import) = syntax_summary.import(item_index) else {
                        continue;
                    };
                    hir_module.imports.push(Import {
                        module,
                        path: import.path,
                        path_spans: import.path_spans,
                        alias: import.alias,
                        alias_span: import.alias_span,
                        span: import.span,
                        resolution: None,
                    });
                }
                SyntaxKind::ConstItem => {
                    let Some((name, visibility, name_span, span)) =
                        syntax_summary.declaration(item_index, DeclarationKind::Const)
                    else {
                        continue;
                    };
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        name,
                        DeclarationKind::Const,
                        visibility,
                        name_span,
                        span,
                    );
                    self.const_metadata.insert(
                        declaration,
                        syntax_metadata::const_metadata(&syntax_summary, item_index),
                    );
                    self.declaration_attrs.insert(
                        declaration,
                        syntax_metadata::attrs(&syntax_summary, item_index),
                    );
                    self.diagnostics
                        .extend(syntax_metadata::const_initializer_diagnostics(
                            &syntax_summary,
                            item_index,
                        ));
                    if let Some(initializer) = syntax_summary.const_initializer_source(item_index) {
                        const_initializers.push(body_binding::ExpressionBodySource::new(
                            declaration,
                            initializer,
                        ));
                    }
                }
                SyntaxKind::GlobalItem => {
                    let Some((name, visibility, name_span, span)) =
                        syntax_summary.declaration(item_index, DeclarationKind::Global)
                    else {
                        continue;
                    };
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        name,
                        DeclarationKind::Global,
                        visibility,
                        name_span,
                        span,
                    );
                    if let Some(metadata) =
                        syntax_metadata::global_metadata(&syntax_summary, item_index)
                    {
                        self.global_metadata.insert(declaration, metadata);
                    }
                    self.declaration_attrs.insert(
                        declaration,
                        syntax_metadata::attrs(&syntax_summary, item_index),
                    );
                }
                SyntaxKind::FunctionItem => {
                    let Some((name, visibility, name_span, span)) =
                        syntax_summary.declaration(item_index, DeclarationKind::Function)
                    else {
                        continue;
                    };
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        name,
                        DeclarationKind::Function,
                        visibility,
                        name_span,
                        span,
                    );
                    let signature =
                        syntax_metadata::function_signature(&syntax_summary, item_index);
                    self.function_signatures
                        .insert(declaration, signature.clone());
                    self.declaration_attrs.insert(
                        declaration,
                        syntax_metadata::attrs(&syntax_summary, item_index),
                    );
                    if let Some(body) = syntax_summary.function_body_source(item_index) {
                        function_declarations.push(FunctionBodySource::new(
                            declaration,
                            signature.params,
                            body,
                        ));
                    }
                }
                SyntaxKind::StructItem => {
                    let Some((name, visibility, name_span, span)) =
                        syntax_summary.declaration(item_index, DeclarationKind::Struct)
                    else {
                        continue;
                    };
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        name,
                        DeclarationKind::Struct,
                        visibility,
                        name_span,
                        span,
                    );
                    let shape = syntax_metadata::struct_shape(&syntax_summary, item_index);
                    self.validate_struct_shape(&shape);
                    self.struct_shapes.insert(declaration, shape);
                    schema_field_defaults.extend(
                        syntax_summary
                            .struct_field_default_sources(item_index)
                            .into_iter()
                            .map(|source| SchemaFieldDefaultBodySource::new(declaration, source)),
                    );
                    self.declaration_attrs.insert(
                        declaration,
                        syntax_metadata::attrs(&syntax_summary, item_index),
                    );
                }
                SyntaxKind::EnumItem => {
                    let Some((name, visibility, name_span, span)) =
                        syntax_summary.declaration(item_index, DeclarationKind::Enum)
                    else {
                        continue;
                    };
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        name,
                        DeclarationKind::Enum,
                        visibility,
                        name_span,
                        span,
                    );
                    let shape = syntax_metadata::enum_shape(&syntax_summary, item_index);
                    self.validate_enum_shape(&shape);
                    self.enum_shapes.insert(declaration, shape);
                    schema_field_defaults.extend(
                        syntax_summary
                            .enum_field_default_sources(item_index)
                            .into_iter()
                            .map(|source| SchemaFieldDefaultBodySource::new(declaration, source)),
                    );
                    self.declaration_attrs.insert(
                        declaration,
                        syntax_metadata::attrs(&syntax_summary, item_index),
                    );
                }
                SyntaxKind::TraitItem => {
                    let Some((name, visibility, name_span, span)) =
                        syntax_summary.declaration(item_index, DeclarationKind::Trait)
                    else {
                        continue;
                    };
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        name,
                        DeclarationKind::Trait,
                        visibility,
                        name_span,
                        span,
                    );
                    let default_method_bodies =
                        syntax_summary.trait_default_body_sources(item_index);
                    let default_method_nodes = default_method_bodies
                        .iter()
                        .map(|body| {
                            body.as_ref()
                                .map(|body| (self.next_node_id(), body.body_span(source)))
                        })
                        .collect::<Vec<_>>();
                    let shape = syntax_metadata::trait_shape(
                        &syntax_summary,
                        item_index,
                        default_method_nodes.clone(),
                    );
                    self.validate_trait_shape(&shape);
                    self.declaration_attrs.insert(
                        declaration,
                        syntax_metadata::attrs(&syntax_summary, item_index),
                    );
                    trait_default_method_declarations.extend(
                        shape
                            .methods
                            .iter()
                            .zip(default_method_nodes)
                            .enumerate()
                            .filter_map(|(method_index, (method_metadata, default_body))| {
                                let (node, _) = default_body?;
                                let body = default_method_bodies.get(method_index)?.clone()?;
                                Some((
                                    node,
                                    FunctionBodySource::new(
                                        declaration,
                                        method_metadata.signature.params.clone(),
                                        body,
                                    ),
                                ))
                            }),
                    );
                    self.trait_shapes.insert(declaration, shape);
                }
                SyntaxKind::ImplItem => {
                    let Some((name, visibility, name_span, span)) =
                        syntax_summary.declaration(item_index, DeclarationKind::Impl)
                    else {
                        continue;
                    };
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        name,
                        DeclarationKind::Impl,
                        visibility,
                        name_span,
                        span,
                    );
                    let method_bodies = syntax_summary.impl_method_body_sources(item_index);
                    let method_nodes = method_bodies
                        .iter()
                        .map(|body| (self.next_node_id(), body.body_span(source)))
                        .collect::<Vec<_>>();
                    let metadata = syntax_metadata::impl_metadata(
                        &syntax_summary,
                        item_index,
                        method_nodes.clone(),
                    );
                    self.validate_impl_shape(&metadata);
                    self.declaration_attrs.insert(
                        declaration,
                        syntax_metadata::attrs(&syntax_summary, item_index),
                    );
                    impl_method_declarations.extend(
                        metadata.methods.iter().enumerate().filter_map(
                            |(method_index, method_metadata)| {
                                let body = method_bodies.get(method_index)?.clone();
                                Some((
                                    method_metadata.node,
                                    FunctionBodySource::new(
                                        declaration,
                                        method_metadata.signature.params.clone(),
                                        body,
                                    ),
                                ))
                            },
                        ),
                    );
                    self.impl_metadata.insert(declaration, metadata);
                }
                _ => {}
            }
        }

        self.validate_import_bindings(&hir_module);

        for source in const_initializers {
            self.bind_const_initializer_body(&hir_module, source);
        }
        for source in schema_field_defaults {
            self.bind_schema_field_default_body(&hir_module, source);
        }
        for source in function_declarations {
            self.bind_function_body(&hir_module, source);
        }
        for (node, source) in trait_default_method_declarations {
            self.bind_trait_default_method_body(&hir_module, node, source);
        }
        for (node, source) in impl_method_declarations {
            self.bind_impl_method_body(&hir_module, node, source);
        }

        self.schema_references_validated = false;
        self.modules.push(hir_module);
        module
    }

    pub fn resolve_imports(&mut self) {
        for module_index in 0..self.modules.len() {
            let import_count = self.modules[module_index].imports.len();
            for import_index in 0..import_count {
                let importing_module = self.modules[module_index].id;
                let import_path = self.modules[module_index].imports[import_index]
                    .path
                    .clone();
                let span = self.modules[module_index].imports[import_index].span;
                let resolution = self.resolve_import_path(importing_module, &import_path, span);
                self.modules[module_index].imports[import_index].resolution = resolution;
            }
        }
        self.refresh_import_binding_resolutions();
        schema_diagnostics::validate_once(self);
    }

    fn insert_declaration(
        &mut self,
        module: &mut HirModule,
        name: String,
        kind: DeclarationKind,
        visibility: Visibility,
        name_span: Span,
        span: Span,
    ) -> HirDeclId {
        let id = self.next_decl_id();
        let node = self.next_node_id();
        let declaration = Declaration {
            id,
            node,
            module: module.id,
            name: name.clone(),
            kind,
            visibility,
            name_span,
            span,
        };

        if let Some(previous_id) = module.declarations.insert(name.clone(), id)
            && let Some(previous) = self.declarations.get(&previous_id)
        {
            self.diagnostics.push(
                Diagnostic::error(format!("duplicate declaration `{name}`"))
                    .with_code("hir::duplicate_declaration")
                    .with_span(name_span)
                    .with_label(previous.name_span, "previous declaration is here")
                    .with_label(name_span, "duplicate declaration is here"),
            );
        }

        self.declarations_by_name
            .entry(name)
            .or_default()
            .insert(id);
        self.declarations_by_kind
            .entry(kind)
            .or_default()
            .insert(id);
        self.declarations.insert(id, declaration);
        id
    }

    fn index_module_path(&mut self, key: &ModuleKey) {
        let segments = key.path.segments();
        for index in 0..segments.len() {
            let parent = ModulePath::new(segments[..index].iter().cloned());
            self.module_children
                .entry(ModuleKey::new(key.package.clone(), parent))
                .or_default()
                .insert(segments[index].clone());
        }
    }

    fn collect_module_completion_labels(
        &self,
        base: &ModuleKey,
        label_prefix: String,
        labels: &mut BTreeSet<String>,
    ) {
        let Some(children) = self.module_children.get(base) else {
            return;
        };
        for child in children {
            let label = if label_prefix.is_empty() {
                child.clone()
            } else {
                format!("{label_prefix}::{child}")
            };
            labels.insert(label.clone());
            let mut child_path = base.path.segments().to_vec();
            child_path.push(child.clone());
            self.collect_module_completion_labels(
                &ModuleKey::new(base.package.clone(), ModulePath::new(child_path)),
                label,
                labels,
            );
        }
    }

    fn module_imports_module(&self, module: &HirModule, imported_module: ModuleId) -> bool {
        module.imports.iter().any(|import| {
            let Some(ImportResolution::Declaration(declaration)) = import.resolution else {
                return false;
            };
            self.declaration(declaration)
                .is_some_and(|declaration| declaration.module == imported_module)
        })
    }

    fn refresh_import_binding_resolutions(&mut self) {
        let imports_by_module = self
            .modules
            .iter()
            .map(|module| {
                let imports = module
                    .imports
                    .iter()
                    .filter_map(|import| {
                        let name = import_binding_name(import)?;
                        let ImportResolution::Declaration(declaration) = import.resolution?;
                        Some((name, declaration))
                    })
                    .collect::<BTreeMap<_, _>>();
                (module.id, imports)
            })
            .collect::<BTreeMap<_, _>>();

        let function_bindings = self
            .bindings
            .keys()
            .filter_map(|declaration| {
                let module = self.declarations.get(declaration)?.module;
                let imports = imports_by_module.get(&module)?.clone();
                Some((*declaration, imports))
            })
            .collect::<Vec<_>>();
        for (declaration, imports) in function_bindings {
            if let Some(bindings) = self.bindings.get_mut(&declaration) {
                bindings.resolve_import_declarations(&imports);
            }
        }

        let const_initializer_bindings = self
            .const_initializer_bindings
            .keys()
            .filter_map(|declaration| {
                let module = self.declarations.get(declaration)?.module;
                let imports = imports_by_module.get(&module)?.clone();
                Some((*declaration, imports))
            })
            .collect::<Vec<_>>();
        for (declaration, imports) in const_initializer_bindings {
            if let Some(bindings) = self.const_initializer_bindings.get_mut(&declaration) {
                bindings.resolve_import_declarations(&imports);
            }
        }

        let trait_default_method_bindings = self
            .trait_default_method_bindings
            .iter()
            .filter_map(|(method, bindings)| {
                let module = self.declarations.get(&bindings.declaration)?.module;
                let imports = imports_by_module.get(&module)?.clone();
                Some((*method, imports))
            })
            .collect::<Vec<_>>();
        for (method, imports) in trait_default_method_bindings {
            if let Some(bindings) = self.trait_default_method_bindings.get_mut(&method) {
                bindings.resolve_import_declarations(&imports);
            }
        }

        let impl_method_bindings = self
            .impl_method_bindings
            .iter()
            .filter_map(|(method, bindings)| {
                let module = self.declarations.get(&bindings.declaration)?.module;
                let imports = imports_by_module.get(&module)?.clone();
                Some((*method, imports))
            })
            .collect::<Vec<_>>();
        for (method, imports) in impl_method_bindings {
            if let Some(bindings) = self.impl_method_bindings.get_mut(&method) {
                bindings.resolve_import_declarations(&imports);
            }
        }

        self.refresh_qualified_binding_resolutions();
    }

    fn refresh_qualified_binding_resolutions(&mut self) {
        let function_bindings = self
            .bindings
            .keys()
            .filter_map(|declaration| {
                let module = self.declarations.get(declaration)?.module;
                let declarations = self.qualified_declarations_for(module);
                Some((*declaration, declarations))
            })
            .collect::<Vec<_>>();
        for (declaration, declarations) in function_bindings {
            if let Some(bindings) = self.bindings.get_mut(&declaration) {
                bindings.resolve_qualified_declarations(&declarations);
            }
        }

        let const_initializer_bindings = self
            .const_initializer_bindings
            .keys()
            .filter_map(|declaration| {
                let module = self.declarations.get(declaration)?.module;
                let declarations = self.qualified_declarations_for(module);
                Some((*declaration, declarations))
            })
            .collect::<Vec<_>>();
        for (declaration, declarations) in const_initializer_bindings {
            if let Some(bindings) = self.const_initializer_bindings.get_mut(&declaration) {
                bindings.resolve_qualified_declarations(&declarations);
            }
        }

        let trait_default_method_bindings = self
            .trait_default_method_bindings
            .iter()
            .filter_map(|(method, bindings)| {
                let module = self.declarations.get(&bindings.declaration)?.module;
                let declarations = self.qualified_declarations_for(module);
                Some((*method, declarations))
            })
            .collect::<Vec<_>>();
        for (method, declarations) in trait_default_method_bindings {
            if let Some(bindings) = self.trait_default_method_bindings.get_mut(&method) {
                bindings.resolve_qualified_declarations(&declarations);
            }
        }

        let impl_method_bindings = self
            .impl_method_bindings
            .iter()
            .filter_map(|(method, bindings)| {
                let module = self.declarations.get(&bindings.declaration)?.module;
                let declarations = self.qualified_declarations_for(module);
                Some((*method, declarations))
            })
            .collect::<Vec<_>>();
        for (method, declarations) in impl_method_bindings {
            if let Some(bindings) = self.impl_method_bindings.get_mut(&method) {
                bindings.resolve_qualified_declarations(&declarations);
            }
        }
    }

    fn lookup_import_declaration(
        &self,
        requesting_key: &ModuleKey,
        requesting_module: ModuleId,
        path: &[String],
    ) -> Option<HirDeclId> {
        let (name, module_segments) = path.split_last()?;
        let module_key = self.import_module_key_from(requesting_key, module_segments);
        let module_id = self.module_by_key.get(&module_key).copied()?;
        let declaration = self
            .module(module_id)
            .and_then(|declarations| declarations.get(name))?;
        self.declaration_visible_from(declaration, requesting_module)
            .then_some(declaration)
    }

    fn resolve_import_path(
        &mut self,
        requesting_module: ModuleId,
        path: &[String],
        span: Span,
    ) -> Option<ImportResolution> {
        let Some((name, module_segments)) = path.split_last() else {
            self.diagnostics.push(
                Diagnostic::error("empty import path")
                    .with_code("hir::empty_import")
                    .with_span(span),
            );
            return None;
        };
        let Some(module_key) = self.import_module_key(requesting_module, module_segments) else {
            self.diagnostics.push(
                Diagnostic::error(
                    "import does not name the current package or a direct dependency",
                )
                .with_code("hir::unknown_package_alias")
                .with_span(span),
            );
            return None;
        };
        let Some(module_id) = self.module_by_key.get(&module_key).copied() else {
            self.diagnostics.push(
                Diagnostic::error(format!("unresolved module `{}`", module_key.path.join()))
                    .with_code("hir::unresolved_module")
                    .with_span(span)
                    .with_label(span, self.module_candidate_label(&module_key)),
            );
            return None;
        };

        let declaration = self
            .module(module_id)
            .and_then(|declarations| declarations.get(name));
        match declaration {
            Some(declaration) if self.declaration_visible_from(declaration, requesting_module) => {
                Some(ImportResolution::Declaration(declaration))
            }
            Some(declaration) => {
                let metadata = self.declaration(declaration)?;
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "declaration `{}` in module `{}` is private",
                        metadata.name,
                        module_key.path.join()
                    ))
                    .with_code("hir::private_import")
                    .with_span(span)
                    .with_label(
                        span,
                        "private declaration cannot be imported from another module",
                    )
                    .with_label(metadata.span, "declaration is private"),
                );
                None
            }
            None => {
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "unresolved import `{}` in module `{}`",
                        name,
                        module_key.path.join()
                    ))
                    .with_code("hir::unresolved_import")
                    .with_span(span)
                    .with_label(span, self.declaration_candidate_label(module_id, name)),
                );
                None
            }
        }
    }

    fn declaration_visible_from(
        &self,
        declaration: HirDeclId,
        requesting_module: ModuleId,
    ) -> bool {
        self.declaration(declaration).is_some_and(|declaration| {
            declaration.module == requesting_module || declaration.visibility == Visibility::Public
        })
    }

    fn declaration_candidate_label(&self, module: ModuleId, name: &str) -> String {
        let Some(declarations) = self.module(module) else {
            return "no declarations are available in this module".to_owned();
        };
        if let Some(candidate) = closest_name(name, declarations.names()) {
            format!("did you mean `{candidate}`?")
        } else {
            "no similar declarations found".to_owned()
        }
    }

    fn module_candidate_label(&self, key: &ModuleKey) -> String {
        let wanted = key.path.join();
        let candidates = self
            .module_by_key
            .keys()
            .filter(|candidate| candidate.package == key.package)
            .map(|candidate| candidate.path.join())
            .collect::<Vec<_>>();
        if let Some(candidate) = closest_name(&wanted, candidates.iter().map(String::as_str)) {
            format!("did you mean module `{candidate}`?")
        } else {
            "no similar modules found".to_owned()
        }
    }

    fn import_module_key(
        &self,
        requesting_module: ModuleId,
        module_segments: &[String],
    ) -> Option<ModuleKey> {
        let requesting = self
            .modules
            .get(usize::try_from(requesting_module.get()).ok()?)?;
        Some(self.import_module_key_from(&requesting.key, module_segments))
    }

    fn import_module_key_from(
        &self,
        requesting: &ModuleKey,
        module_segments: &[String],
    ) -> ModuleKey {
        let (package, path_segments) = match module_segments.split_first() {
            Some((first, rest)) if first == "crate" => (requesting.package.clone(), rest),
            Some((first, rest)) => {
                let alias = PackageAlias::new(first).ok();
                match alias.and_then(|alias| {
                    self.package_dependencies
                        .get(&requesting.package)
                        .and_then(|dependencies| dependencies.get(&alias))
                }) {
                    Some(package) => (package.clone(), rest),
                    None => (requesting.package.clone(), module_segments),
                }
            }
            None => (requesting.package.clone(), module_segments),
        };
        ModuleKey::new(package, ModulePath::new(path_segments.iter().cloned()))
    }

    fn next_module_id(&self) -> ModuleId {
        ModuleId::new(u32::try_from(self.modules.len()).unwrap_or(u32::MAX))
    }

    fn next_node_id(&mut self) -> HirNodeId {
        let id = HirNodeId::new(self.next_node_id);
        self.next_node_id = self.next_node_id.saturating_add(1);
        id
    }

    fn next_decl_id(&mut self) -> HirDeclId {
        let id = HirDeclId::new(self.next_decl_id);
        self.next_decl_id = self.next_decl_id.saturating_add(1);
        id
    }

    fn next_body_id(&mut self) -> HirBodyId {
        let id = HirBodyId::new(self.next_body_id);
        self.next_body_id = self.next_body_id.saturating_add(1);
        id
    }

    fn extend_bodies(&mut self, bodies: impl IntoIterator<Item = HirBody>) {
        for body in bodies {
            self.body_ids_by_source
                .entry(body.origin.source)
                .or_default()
                .insert(body.id);
            self.bodies.insert(body.id, body);
        }
    }
}

#[must_use]
pub fn stable_source_hash(text: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    text.as_bytes().iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

#[cfg(test)]
mod tests;
