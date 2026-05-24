use std::collections::{BTreeMap, btree_map::Entry};

mod schema_diagnostics;

use vela_common::{Diagnostic, SourceId, Span};
use vela_syntax::{
    Block, FunctionItem, ItemKind, Param, SourceFile, TraitMethod, Visibility, parse_source,
};

use crate::attributes::{HirAttribute, attrs_from_syntax};
use crate::binding::{BindingMap, FunctionBindingInput, ImportBinding, bind_function};
use crate::top_level::validate_const_initializer;
use crate::type_hint::{
    ConstMetadata, EnumShape, FunctionSignature, HirTypeHint, ImplMetadata, ParamHint,
    StructFieldHint, StructShape, TraitShape,
};
use crate::{HirDeclId, HirNodeId, ModuleId};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModulePath(Vec<String>);

impl ModulePath {
    #[must_use]
    pub fn new(segments: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(segments.into_iter().map(Into::into).collect())
    }

    #[must_use]
    pub fn from_dotted(path: &str) -> Self {
        Self::new(path.split('.').filter(|segment| !segment.is_empty()))
    }

    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.0
    }

    #[must_use]
    pub fn join(&self) -> String {
        self.0.join(".")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleSource {
    pub id: SourceId,
    pub path: ModulePath,
    pub text: String,
}

impl ModuleSource {
    #[must_use]
    pub fn new(id: SourceId, path: ModulePath, text: impl Into<String>) -> Self {
        Self {
            id,
            path,
            text: text.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    pub id: HirDeclId,
    pub node: HirNodeId,
    pub module: ModuleId,
    pub name: String,
    pub kind: DeclarationKind,
    pub visibility: Visibility,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationKind {
    Const,
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Import {
    pub module: ModuleId,
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub span: Span,
    pub resolution: Option<ImportResolution>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportResolution {
    Declaration(HirDeclId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImport {
    pub path: Vec<String>,
    pub resolution: ImportResolution,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeclarationIndex {
    by_name: BTreeMap<String, HirDeclId>,
}

impl DeclarationIndex {
    #[must_use]
    pub fn get(&self, name: &str) -> Option<HirDeclId> {
        self.by_name.get(name).copied()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(String::as_str)
    }

    fn insert(&mut self, name: String, id: HirDeclId) -> Option<HirDeclId> {
        match self.by_name.entry(name) {
            Entry::Vacant(entry) => {
                entry.insert(id);
                None
            }
            Entry::Occupied(entry) => Some(*entry.get()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HirModule {
    id: ModuleId,
    path: ModulePath,
    source: SourceId,
    declarations: DeclarationIndex,
    imports: Vec<Import>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleGraph {
    modules: Vec<HirModule>,
    module_by_path: BTreeMap<ModulePath, ModuleId>,
    declarations: BTreeMap<HirDeclId, Declaration>,
    declaration_attrs: BTreeMap<HirDeclId, Vec<HirAttribute>>,
    const_metadata: BTreeMap<HirDeclId, ConstMetadata>,
    bindings: BTreeMap<HirDeclId, BindingMap>,
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
    next_expr_id: u32,
    next_local_id: u32,
}

impl ModuleGraph {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_source(&mut self, source: ModuleSource) -> ModuleId {
        let parsed = parse_source(source.id, &source.text);
        self.add_parsed_source(source.id, source.path, parsed)
    }

    pub fn add_parsed_source(
        &mut self,
        source: SourceId,
        path: ModulePath,
        parsed: SourceFile,
    ) -> ModuleId {
        let module = self.next_module_id();
        let module_span = self.module_span(source, &parsed);

        if let Some(existing) = self.module_by_path.get(&path).copied() {
            self.diagnostics.push(
                Diagnostic::error(format!("duplicate module `{}`", path.join()))
                    .with_code("hir::duplicate_module")
                    .with_label(
                        module_span,
                        format!("module `{}` is declared more than once", path.join()),
                    ),
            );
            self.diagnostics.extend(parsed.diagnostics);
            return existing;
        }
        self.module_by_path.insert(path.clone(), module);
        self.diagnostics.extend(parsed.diagnostics);

        let mut hir_module = HirModule {
            id: module,
            path,
            source,
            declarations: DeclarationIndex::default(),
            imports: Vec::new(),
        };

        let mut function_declarations = Vec::new();
        let mut trait_default_method_declarations = Vec::new();
        let mut impl_method_declarations = Vec::new();

        for item in &parsed.items {
            match &item.kind {
                ItemKind::Use(use_item) => {
                    hir_module.imports.push(Import {
                        module,
                        path: use_item.path.clone(),
                        alias: use_item.alias.clone(),
                        span: item.span,
                        resolution: None,
                    });
                }
                ItemKind::Const(const_item) => {
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        const_item.name.clone(),
                        DeclarationKind::Const,
                        item.visibility.clone(),
                        item.span,
                    );
                    self.const_metadata
                        .insert(declaration, ConstMetadata::from_syntax(const_item));
                    self.declaration_attrs
                        .insert(declaration, attrs_from_syntax(&item.attrs));
                    self.diagnostics
                        .extend(validate_const_initializer(const_item));
                }
                ItemKind::Function(function) => {
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        function.name.clone(),
                        DeclarationKind::Function,
                        item.visibility.clone(),
                        item.span,
                    );
                    self.function_signatures.insert(
                        declaration,
                        FunctionSignature {
                            params: function.params.iter().map(ParamHint::from_syntax).collect(),
                            return_type: function
                                .return_type
                                .as_ref()
                                .map(HirTypeHint::from_syntax),
                        },
                    );
                    self.declaration_attrs
                        .insert(declaration, attrs_from_syntax(&item.attrs));
                    function_declarations.push((declaration, function.clone()));
                }
                ItemKind::Struct(record) => {
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        record.name.clone(),
                        DeclarationKind::Struct,
                        item.visibility.clone(),
                        item.span,
                    );
                    self.struct_shapes.insert(
                        declaration,
                        StructShape {
                            fields: record
                                .fields
                                .iter()
                                .map(StructFieldHint::from_syntax)
                                .collect(),
                        },
                    );
                    self.declaration_attrs
                        .insert(declaration, attrs_from_syntax(&item.attrs));
                }
                ItemKind::Enum(enumeration) => {
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        enumeration.name.clone(),
                        DeclarationKind::Enum,
                        item.visibility.clone(),
                        item.span,
                    );
                    self.enum_shapes
                        .insert(declaration, EnumShape::from_syntax(enumeration));
                    self.declaration_attrs
                        .insert(declaration, attrs_from_syntax(&item.attrs));
                }
                ItemKind::Trait(trait_item) => {
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        trait_item.name.clone(),
                        DeclarationKind::Trait,
                        item.visibility.clone(),
                        item.span,
                    );
                    let default_method_nodes = trait_item
                        .methods
                        .iter()
                        .map(|method| {
                            method
                                .default_body
                                .as_ref()
                                .map(|body| (self.next_node_id(), body.span))
                        })
                        .collect::<Vec<_>>();
                    self.trait_shapes.insert(
                        declaration,
                        TraitShape::from_syntax(trait_item, default_method_nodes.clone()),
                    );
                    self.declaration_attrs
                        .insert(declaration, attrs_from_syntax(&item.attrs));
                    trait_default_method_declarations.extend(
                        trait_item
                            .methods
                            .iter()
                            .zip(default_method_nodes)
                            .filter_map(|(method, default_body)| {
                                default_body.map(|(node, _)| (declaration, node, method.clone()))
                            }),
                    );
                }
                ItemKind::Impl(impl_item) => {
                    let name = impl_declaration_name(&impl_item.trait_path, &impl_item.target_path);
                    let declaration = self.insert_declaration(
                        &mut hir_module,
                        name,
                        DeclarationKind::Impl,
                        item.visibility.clone(),
                        item.span,
                    );
                    let method_nodes = impl_item
                        .methods
                        .iter()
                        .map(|method| (self.next_node_id(), method.function.body.span))
                        .collect::<Vec<_>>();
                    self.impl_metadata.insert(
                        declaration,
                        ImplMetadata::from_syntax(impl_item, method_nodes.clone()),
                    );
                    self.declaration_attrs
                        .insert(declaration, attrs_from_syntax(&item.attrs));
                    impl_method_declarations.extend(
                        impl_item
                            .methods
                            .iter()
                            .zip(method_nodes)
                            .map(|(method, (node, _))| {
                                (declaration, node, method.function.clone())
                            }),
                    );
                }
            }
        }

        for (declaration, function) in function_declarations {
            self.bind_function_body(&hir_module, declaration, &function);
        }
        for (declaration, node, method) in trait_default_method_declarations {
            self.bind_trait_default_method_body(&hir_module, declaration, node, &method);
        }
        for (declaration, node, function) in impl_method_declarations {
            self.bind_impl_method_body(&hir_module, declaration, node, &function);
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

    #[must_use]
    pub fn module(&self, module: ModuleId) -> Option<&DeclarationIndex> {
        self.modules
            .get(usize::try_from(module.get()).ok()?)
            .map(|module| &module.declarations)
    }

    #[must_use]
    pub fn module_path(&self, module: ModuleId) -> Option<&ModulePath> {
        self.modules
            .get(usize::try_from(module.get()).ok()?)
            .map(|module| &module.path)
    }

    #[must_use]
    pub fn declaration(&self, declaration: HirDeclId) -> Option<&Declaration> {
        self.declarations.get(&declaration)
    }

    #[must_use]
    pub fn const_metadata(&self, declaration: HirDeclId) -> Option<&ConstMetadata> {
        self.const_metadata.get(&declaration)
    }

    #[must_use]
    pub fn declaration_attrs(&self, declaration: HirDeclId) -> &[HirAttribute] {
        self.declaration_attrs
            .get(&declaration)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn bindings(&self, declaration: HirDeclId) -> Option<&BindingMap> {
        self.bindings.get(&declaration)
    }

    #[must_use]
    pub fn function_signature(&self, declaration: HirDeclId) -> Option<&FunctionSignature> {
        self.function_signatures.get(&declaration)
    }

    #[must_use]
    pub fn struct_shape(&self, declaration: HirDeclId) -> Option<&StructShape> {
        self.struct_shapes.get(&declaration)
    }

    #[must_use]
    pub fn enum_shape(&self, declaration: HirDeclId) -> Option<&EnumShape> {
        self.enum_shapes.get(&declaration)
    }

    #[must_use]
    pub fn trait_shape(&self, declaration: HirDeclId) -> Option<&TraitShape> {
        self.trait_shapes.get(&declaration)
    }

    pub fn declarations(&self) -> impl Iterator<Item = &Declaration> {
        self.declarations.values()
    }

    #[must_use]
    pub fn impl_metadata(&self, declaration: HirDeclId) -> Option<&ImplMetadata> {
        self.impl_metadata.get(&declaration)
    }

    #[must_use]
    pub fn trait_default_method_bindings(&self, method: HirNodeId) -> Option<&BindingMap> {
        self.trait_default_method_bindings.get(&method)
    }

    #[must_use]
    pub fn impl_method_bindings(&self, method: HirNodeId) -> Option<&BindingMap> {
        self.impl_method_bindings.get(&method)
    }

    #[must_use]
    pub fn imports(&self, module: ModuleId) -> Option<&[Import]> {
        self.modules
            .get(usize::try_from(module.get()).ok()?)
            .map(|module| module.imports.as_slice())
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    fn insert_declaration(
        &mut self,
        module: &mut HirModule,
        name: String,
        kind: DeclarationKind,
        visibility: Visibility,
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
            span,
        };

        if let Some(previous_id) = module.declarations.insert(name.clone(), id)
            && let Some(previous) = self.declarations.get(&previous_id)
        {
            self.diagnostics.push(
                Diagnostic::error(format!("duplicate declaration `{name}`"))
                    .with_code("hir::duplicate_declaration")
                    .with_span(span)
                    .with_label(previous.span, "previous declaration is here")
                    .with_label(span, "duplicate declaration is here"),
            );
        }

        self.declarations.insert(id, declaration);
        id
    }

    fn bind_function_body(
        &mut self,
        module: &HirModule,
        declaration: HirDeclId,
        function: &FunctionItem,
    ) {
        let (bindings, diagnostics) =
            self.bind_body(module, declaration, &function.params, &function.body);
        self.bindings.insert(declaration, bindings);
        self.diagnostics.extend(diagnostics);
    }

    fn bind_trait_default_method_body(
        &mut self,
        module: &HirModule,
        declaration: HirDeclId,
        method: HirNodeId,
        trait_method: &TraitMethod,
    ) {
        let Some(body) = &trait_method.default_body else {
            return;
        };
        let (bindings, diagnostics) =
            self.bind_body(module, declaration, &trait_method.params, body);
        self.trait_default_method_bindings.insert(method, bindings);
        self.diagnostics.extend(diagnostics);
    }

    fn bind_impl_method_body(
        &mut self,
        module: &HirModule,
        declaration: HirDeclId,
        method: HirNodeId,
        function: &FunctionItem,
    ) {
        let (bindings, diagnostics) =
            self.bind_body(module, declaration, &function.params, &function.body);
        self.impl_method_bindings.insert(method, bindings);
        self.diagnostics.extend(diagnostics);
    }

    fn bind_body(
        &mut self,
        module: &HirModule,
        declaration: HirDeclId,
        params: &[Param],
        body: &Block,
    ) -> (BindingMap, Vec<Diagnostic>) {
        let module_declarations = module
            .declarations
            .names()
            .filter_map(|name| {
                module
                    .declarations
                    .get(name)
                    .map(|declaration| (name.to_owned(), declaration))
            })
            .collect::<Vec<_>>();
        let imports = self.import_bindings(module);
        let qualified_declarations = self.qualified_declarations_with(module);

        bind_function(FunctionBindingInput {
            declaration,
            params,
            body,
            module_declarations,
            qualified_declarations,
            imports,
            next_expr_id: &mut self.next_expr_id,
            next_local_id: &mut self.next_local_id,
        })
    }

    fn import_bindings(&self, module: &HirModule) -> Vec<ImportBinding> {
        module
            .imports
            .iter()
            .filter_map(|import| {
                let name = import_binding_name(import)?;
                let declaration = match import.resolution {
                    Some(ImportResolution::Declaration(declaration)) => Some(declaration),
                    None => self.lookup_import_declaration(import.module, &import.path),
                };
                Some(ImportBinding { name, declaration })
            })
            .collect()
    }

    fn qualified_declarations_with(&self, current: &HirModule) -> Vec<(Vec<String>, HirDeclId)> {
        let mut declarations = self.qualified_declarations_for(current.id);
        declarations.extend(self.qualified_declarations_in(current, current.id));
        declarations.into_iter().collect()
    }

    fn qualified_declarations_for(
        &self,
        requesting_module: ModuleId,
    ) -> BTreeMap<Vec<String>, HirDeclId> {
        self.modules
            .iter()
            .flat_map(|module| self.qualified_declarations_in(module, requesting_module))
            .collect()
    }

    fn qualified_declarations_in(
        &self,
        module: &HirModule,
        requesting_module: ModuleId,
    ) -> Vec<(Vec<String>, HirDeclId)> {
        module
            .declarations
            .names()
            .filter_map(|name| {
                let declaration = module.declarations.get(name)?;
                if !self.declaration_visible_from(declaration, requesting_module) {
                    return None;
                }
                let mut path = module.path.segments().to_vec();
                path.push(name.to_owned());
                Some((path, declaration))
            })
            .collect()
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
        requesting_module: ModuleId,
        path: &[String],
    ) -> Option<HirDeclId> {
        let (name, module_segments) = path.split_last()?;
        let module_path = ModulePath::new(module_segments.iter().cloned());
        let module_id = self.module_by_path.get(&module_path).copied()?;
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
        let module_path = ModulePath::new(module_segments.iter().cloned());
        let Some(module_id) = self.module_by_path.get(&module_path).copied() else {
            self.diagnostics.push(
                Diagnostic::error(format!("unresolved module `{}`", module_path.join()))
                    .with_code("hir::unresolved_module")
                    .with_span(span)
                    .with_label(span, self.module_candidate_label(&module_path)),
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
                        module_path.join()
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
                        module_path.join()
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

    fn module_candidate_label(&self, path: &ModulePath) -> String {
        let wanted = path.join();
        let candidates = self
            .module_by_path
            .keys()
            .map(ModulePath::join)
            .collect::<Vec<_>>();
        if let Some(candidate) = closest_name(&wanted, candidates.iter().map(String::as_str)) {
            format!("did you mean module `{candidate}`?")
        } else {
            "no similar modules found".to_owned()
        }
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

    fn module_span(&self, source: SourceId, parsed: &SourceFile) -> Span {
        parsed
            .items
            .first()
            .map_or_else(|| Span::new(source, 0, 0), |item| item.span)
    }
}

fn closest_name(
    wanted: &str,
    candidates: impl IntoIterator<Item = impl AsRef<str>>,
) -> Option<String> {
    candidates
        .into_iter()
        .map(|candidate| candidate.as_ref().to_owned())
        .min_by_key(|candidate| candidate_distance(wanted, candidate))
        .filter(|candidate| candidate_distance(wanted, candidate) <= 3)
}

fn impl_declaration_name(trait_path: &[String], target_path: &[String]) -> String {
    format!(
        "impl {} for {}",
        trait_path.join("."),
        target_path.join(".")
    )
}

fn import_binding_name(import: &Import) -> Option<String> {
    import.alias.clone().or_else(|| import.path.last().cloned())
}

fn candidate_distance(wanted: &str, candidate: &str) -> usize {
    if wanted.contains(candidate) || candidate.contains(wanted) {
        return 0;
    }
    levenshtein(wanted, candidate)
}

fn levenshtein(lhs: &str, rhs: &str) -> usize {
    let mut previous = (0..=rhs.chars().count()).collect::<Vec<_>>();
    let mut current = vec![0; previous.len()];

    for (lhs_index, lhs_char) in lhs.chars().enumerate() {
        current[0] = lhs_index + 1;
        for (rhs_index, rhs_char) in rhs.chars().enumerate() {
            let cost = usize::from(lhs_char != rhs_char);
            current[rhs_index + 1] = (previous[rhs_index + 1] + 1)
                .min(current[rhs_index] + 1)
                .min(previous[rhs_index] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[rhs.chars().count()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BindingResolution, LocalBindingKind};

    fn source(id: u32, module: &str, text: &str) -> ModuleSource {
        ModuleSource::new(SourceId::new(id), ModulePath::from_dotted(module), text)
    }

    #[test]
    fn indexes_top_level_declarations_with_stable_ids() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.reward",
            r#"
pub fn grant(player) { return player; }
pub const START_LEVEL: int = 1 + 2;
struct Reward { item_id, count }
enum QuestProgress { None, Active }
trait Damageable { fn damage(self, amount); }
"#,
        ));

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let declarations = graph.module(module).expect("module declarations");
        let grant = declarations.get("grant").expect("grant declaration");
        let start_level = declarations.get("START_LEVEL").expect("const declaration");
        let reward = declarations.get("Reward").expect("Reward declaration");

        assert_ne!(grant, reward);
        assert_eq!(grant.get(), 0);
        assert_eq!(start_level.get(), 1);
        assert_eq!(reward.get(), 2);
        assert_eq!(
            graph.declaration(grant).map(|decl| decl.kind),
            Some(DeclarationKind::Function)
        );
        assert_eq!(
            graph.declaration(start_level).map(|decl| decl.kind),
            Some(DeclarationKind::Const)
        );
        assert_eq!(
            graph
                .const_metadata(start_level)
                .and_then(|metadata| metadata.type_hint.as_ref())
                .map(HirTypeHint::display)
                .as_deref(),
            Some("int")
        );
        assert_eq!(
            graph.declaration(reward).map(|decl| decl.kind),
            Some(DeclarationKind::Struct)
        );
    }

    #[test]
    fn resolves_imports_across_modules() {
        let mut graph = ModuleGraph::new();
        let _reward = graph.add_source(source(1, "game.reward", "pub fn grant() { return 1; }"));
        let main = graph.add_source(source(
            2,
            "game.main",
            r#"
use game.reward.grant
fn main() { return grant(); }
"#,
        ));

        graph.resolve_imports();

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let imports = graph.imports(main).expect("imports");
        let Some(ImportResolution::Declaration(declaration)) =
            imports.first().and_then(|import| import.resolution)
        else {
            panic!("expected resolved declaration import");
        };
        assert_eq!(
            graph
                .declaration(declaration)
                .map(|decl| decl.name.as_str()),
            Some("grant")
        );
    }

    #[test]
    fn duplicate_declarations_report_both_spans() {
        let mut graph = ModuleGraph::new();
        graph.add_source(source(
            1,
            "game.player",
            r#"
fn level() { return 1; }
struct level { value }
"#,
        ));

        let duplicate = graph
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_declaration"))
            .expect("duplicate declaration diagnostic");

        assert_eq!(duplicate.labels.len(), 2);
        assert!(duplicate.labels[0].message.contains("previous"));
        assert!(duplicate.labels[1].message.contains("duplicate"));
    }

    #[test]
    fn unresolved_imports_include_candidate_hints() {
        let mut graph = ModuleGraph::new();
        graph.add_source(source(1, "game.reward", "pub fn grant() { return 1; }"));
        graph.add_source(source(2, "game.main", "use game.reward.grant_reward"));

        graph.resolve_imports();

        let unresolved = graph
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::unresolved_import"))
            .expect("unresolved import diagnostic");

        assert_eq!(unresolved.labels.len(), 1);
        assert!(unresolved.labels[0].message.contains("grant"));
    }

    #[test]
    fn private_imports_are_rejected_across_modules() {
        let mut graph = ModuleGraph::new();
        let reward = graph.add_source(source(1, "game.reward", "fn secret() { return 1; }"));
        let main = graph.add_source(source(2, "game.main", "use game.reward.secret"));

        graph.resolve_imports();

        let private = graph
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::private_import"))
            .expect("private import diagnostic");
        let imports = graph.imports(main).expect("main imports");
        let secret = graph
            .module(reward)
            .and_then(|module| module.get("secret"))
            .expect("secret declaration");

        assert_eq!(imports[0].resolution, None);
        assert_eq!(private.labels.len(), 2);
        assert_eq!(
            graph.declaration(secret).map(|decl| &decl.visibility),
            Some(&Visibility::Private)
        );
    }

    #[test]
    fn function_bindings_resolve_params_and_locals_with_expression_ids() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.player",
            r#"
fn main(player) {
    let next = player.level;
    return next;
}
"#,
        ));
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let bindings = graph.bindings(main).expect("main bindings");
        let [player] = bindings.locals_named("player") else {
            panic!("expected one player binding");
        };
        let [next] = bindings.locals_named("next") else {
            panic!("expected one next binding");
        };

        assert_eq!(
            bindings.local(*player).map(|local| local.kind),
            Some(LocalBindingKind::Parameter)
        );
        assert_eq!(
            bindings.local(*next).map(|local| local.kind),
            Some(LocalBindingKind::Let)
        );
        assert!(bindings.expression_count() >= 2);
        assert!(
            bindings
                .resolutions()
                .any(|(_, resolution)| resolution == &BindingResolution::Local(*player))
        );
        assert!(
            bindings
                .resolutions()
                .any(|(_, resolution)| resolution == &BindingResolution::Local(*next))
        );
    }

    #[test]
    fn binding_unresolved_names_report_candidate_hints() {
        let mut graph = ModuleGraph::new();
        graph.add_source(source(
            1,
            "game.player",
            r#"
fn main(player) {
    return plaeyr;
}
"#,
        ));

        let unresolved = graph
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::unresolved_name"))
            .expect("unresolved name diagnostic");

        assert_eq!(unresolved.labels.len(), 1);
        assert!(unresolved.labels[0].message.contains("player"));
    }

    #[test]
    fn binding_tracks_nested_for_and_lambda_scopes() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.reward",
            r#"
fn main(rewards) {
    for reward in rewards {
        let mapper = |reward| reward.count;
    }
    return rewards;
}
"#,
        ));
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let bindings = graph.bindings(main).expect("main bindings");
        let reward_bindings = bindings.locals_named("reward");

        assert_eq!(reward_bindings.len(), 2);
        assert_eq!(
            bindings.local(reward_bindings[0]).map(|local| local.kind),
            Some(LocalBindingKind::For)
        );
        assert_eq!(
            bindings.local(reward_bindings[1]).map(|local| local.kind),
            Some(LocalBindingKind::LambdaParameter)
        );
    }

    #[test]
    fn function_bindings_resolve_imported_names() {
        let mut graph = ModuleGraph::new();
        let reward = graph.add_source(source(1, "game.reward", "pub fn grant() { return 1; }"));
        let module = graph.add_source(source(
            2,
            "game.main",
            r#"
use game.reward.grant
fn main() { return grant; }
"#,
        ));
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");
        let grant = graph
            .module(reward)
            .and_then(|module| module.get("grant"))
            .expect("grant declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let bindings = graph.bindings(main).expect("main bindings");
        assert!(
            bindings
                .resolutions()
                .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(grant) })
        );
    }

    #[test]
    fn function_bindings_resolve_import_aliases() {
        let mut graph = ModuleGraph::new();
        let reward = graph.add_source(source(1, "game.reward", "pub fn grant() { return 1; }"));
        let module = graph.add_source(source(
            2,
            "game.main",
            r#"
use game.reward.grant as give_reward
fn main() { return give_reward; }
"#,
        ));
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");
        let grant = graph
            .module(reward)
            .and_then(|module| module.get("grant"))
            .expect("grant declaration");
        let imports = graph.imports(module).expect("module imports");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        assert_eq!(imports[0].alias.as_deref(), Some("give_reward"));
        let bindings = graph.bindings(main).expect("main bindings");
        assert!(
            bindings
                .resolutions()
                .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(grant) })
        );
    }

    #[test]
    fn function_bindings_resolve_record_constructor_import_aliases() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.main",
            r#"
use game.reward.Reward as Prize

fn main() {
    return Prize { count: 2 };
}
"#,
        ));
        let reward = graph.add_source(source(
            2,
            "game.reward",
            r#"
pub struct Reward { count: int }
"#,
        ));
        graph.resolve_imports();
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");
        let reward = graph
            .module(reward)
            .and_then(|module| module.get("Reward"))
            .expect("reward declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let bindings = graph.bindings(main).expect("main bindings");
        assert!(
            bindings
                .resolutions()
                .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(reward) })
        );
    }

    #[test]
    fn function_bindings_resolve_match_pattern_import_aliases() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.main",
            r#"
use game.damage.Damage as Hit

fn main(damage) {
    match damage {
        Hit.Physical { amount } => { return amount; },
        _ => { return 0; },
    }
}
"#,
        ));
        let damage = graph.add_source(source(
            2,
            "game.damage",
            r#"
pub enum Damage { Physical }
"#,
        ));
        graph.resolve_imports();
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");
        let damage = graph
            .module(damage)
            .and_then(|module| module.get("Damage"))
            .expect("damage declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let bindings = graph.bindings(main).expect("main bindings");
        assert!(bindings.pattern_resolutions().any(|(path, resolution)| {
            path == ["Hit".to_owned(), "Physical".to_owned()]
                && resolution == &BindingResolution::Declaration(damage)
        }));
    }

    #[test]
    fn function_bindings_resolve_tuple_constructor_call_aliases() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.main",
            r#"
use game.damage.Damage as Hit

fn main() {
    return Hit.Physical(7);
}
"#,
        ));
        let damage = graph.add_source(source(
            2,
            "game.damage",
            r#"
pub enum Damage { Physical(amount) }
"#,
        ));
        graph.resolve_imports();
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");
        let damage = graph
            .module(damage)
            .and_then(|module| module.get("Damage"))
            .expect("damage declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let bindings = graph.bindings(main).expect("main bindings");
        assert!(
            bindings
                .resolutions()
                .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(damage) })
        );
    }

    #[test]
    fn resolved_imports_refresh_existing_binding_maps() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.main",
            r#"
use game.reward.grant
fn main() { return grant; }
"#,
        ));
        let reward = graph.add_source(source(2, "game.reward", "pub fn grant() { return 1; }"));
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");
        let grant = graph
            .module(reward)
            .and_then(|module| module.get("grant"))
            .expect("grant declaration");

        assert!(
            graph
                .bindings(main)
                .expect("main bindings")
                .resolutions()
                .any(|(_, resolution)| {
                    resolution == &BindingResolution::Import("grant".to_owned())
                })
        );

        graph.resolve_imports();

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        assert!(
            graph
                .bindings(main)
                .expect("main bindings")
                .resolutions()
                .any(|(_, resolution)| { resolution == &BindingResolution::Declaration(grant) })
        );
        assert!(
            !graph
                .bindings(main)
                .expect("main bindings")
                .resolutions()
                .any(|(_, resolution)| {
                    resolution == &BindingResolution::Import("grant".to_owned())
                })
        );
    }

    #[test]
    fn resolved_modules_refresh_qualified_path_binding_maps() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.main",
            r#"
fn main() {
    return game.reward.grant() + game.config.BONUS;
}
"#,
        ));
        let reward = graph.add_source(source(
            2,
            "game.reward",
            r#"
pub fn grant() { return 4; }
"#,
        ));
        let config = graph.add_source(source(
            3,
            "game.config",
            r#"
pub const BONUS: int = 5;
"#,
        ));
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");
        let grant = graph
            .module(reward)
            .and_then(|module| module.get("grant"))
            .expect("grant declaration");
        let bonus = graph
            .module(config)
            .and_then(|module| module.get("BONUS"))
            .expect("bonus declaration");

        assert!(
            graph
                .bindings(main)
                .expect("main bindings")
                .resolutions()
                .any(|(_, resolution)| {
                    resolution
                        == &BindingResolution::QualifiedPath(vec![
                            "game".to_owned(),
                            "reward".to_owned(),
                            "grant".to_owned(),
                        ])
                })
        );

        graph.resolve_imports();

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let bindings = graph.bindings(main).expect("main bindings");
        assert!(
            bindings
                .resolutions()
                .any(|(_, resolution)| resolution == &BindingResolution::Declaration(grant))
        );
        assert!(
            bindings
                .resolutions()
                .any(|(_, resolution)| resolution == &BindingResolution::Declaration(bonus))
        );
    }

    #[test]
    fn qualified_private_paths_do_not_resolve_across_modules() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.main",
            r#"
fn main() {
    return game.reward.secret();
}
"#,
        ));
        graph.add_source(source(
            2,
            "game.reward",
            r#"
fn secret() { return 1; }
"#,
        ));
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");

        graph.resolve_imports();

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let bindings = graph.bindings(main).expect("main bindings");
        assert!(bindings.resolutions().any(|(_, resolution)| {
            resolution
                == &BindingResolution::QualifiedPath(vec![
                    "game".to_owned(),
                    "reward".to_owned(),
                    "secret".to_owned(),
                ])
        }));
    }

    #[test]
    fn binding_treats_bare_map_keys_as_keys_not_name_reads() {
        let mut graph = ModuleGraph::new();
        graph.add_source(source(
            1,
            "game.reward",
            r#"
fn main() {
    return { exp: 15 };
}
"#,
        ));

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
    }

    #[test]
    fn binding_resolves_record_shorthand_fields() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.reward",
            r#"
fn main() {
    let count = 2;
    return Reward { count };
}
"#,
        ));
        let main = graph
            .module(module)
            .and_then(|module| module.get("main"))
            .expect("main declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let bindings = graph.bindings(main).expect("main bindings");
        let [count] = bindings.locals_named("count") else {
            panic!("expected count binding");
        };

        assert!(
            bindings
                .resolutions()
                .any(|(_, resolution)| { resolution == &BindingResolution::Local(*count) })
        );
    }

    #[test]
    fn lowers_type_hint_metadata_for_signatures_structs_and_locals() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.reward",
            r#"
fn grant(player: game.Player, amount: int) -> Result {
    let reward: Reward = Reward { count: amount };
    let mapper = |entry: Reward| entry.count;
    return reward;
}

struct Reward {
    count: int,
}
"#,
        ));
        let declarations = graph.module(module).expect("module declarations");
        let grant = declarations.get("grant").expect("grant declaration");
        let reward = declarations.get("Reward").expect("Reward declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let signature = graph.function_signature(grant).expect("function signature");
        assert_eq!(signature.params[0].name, "player");
        assert_eq!(
            signature.params[0]
                .type_hint
                .as_ref()
                .map(HirTypeHint::display)
                .as_deref(),
            Some("game.Player")
        );
        assert_eq!(
            signature
                .return_type
                .as_ref()
                .map(HirTypeHint::display)
                .as_deref(),
            Some("Result")
        );

        let shape = graph.struct_shape(reward).expect("struct shape");
        assert_eq!(shape.fields[0].name, "count");
        assert_eq!(
            shape.fields[0]
                .type_hint
                .as_ref()
                .map(HirTypeHint::display)
                .as_deref(),
            Some("int")
        );

        let bindings = graph.bindings(grant).expect("grant bindings");
        let [reward_local] = bindings.locals_named("reward") else {
            panic!("expected reward local");
        };
        assert_eq!(
            bindings
                .local(*reward_local)
                .and_then(|local| local.type_hint.as_ref())
                .map(HirTypeHint::display)
                .as_deref(),
            Some("Reward")
        );
        let entry_bindings = bindings.locals_named("entry");
        assert_eq!(
            bindings
                .local(entry_bindings[0])
                .and_then(|local| local.type_hint.as_ref())
                .map(HirTypeHint::display)
                .as_deref(),
            Some("Reward")
        );
    }

    #[test]
    fn unknown_schema_type_hints_report_ranked_related_candidates() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.combat",
            r#"
struct Player { hp: int }

fn grant(player: Plyer) {
    return null;
}
"#,
        ));

        graph.resolve_imports();

        let player = graph
            .module(module)
            .and_then(|module| module.get("Player"))
            .and_then(|declaration| graph.declaration(declaration))
            .expect("Player declaration");
        let diagnostics = graph.diagnostics();
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code.as_deref(), Some("hir::unknown_schema"));
        assert_eq!(diagnostic.message, "unknown schema `Plyer`");
        assert_eq!(diagnostic.labels.len(), 2);
        assert_eq!(
            diagnostic.labels[0].message,
            "`Plyer` does not resolve to a known schema"
        );
        assert_eq!(diagnostic.labels[1].span, player.span);
        assert_eq!(
            diagnostic.labels[1].message,
            "candidate `Player` is declared here"
        );
    }

    #[test]
    fn unknown_impl_schema_names_report_trait_and_target_candidates() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.combat",
            r#"
trait Damageable {
    fn damage(self);
}

struct Player { hp: int }

impl Damageabl for Playr {}
"#,
        ));

        graph.resolve_imports();

        let declarations = graph.module(module).expect("module declarations");
        let damageable = declarations
            .get("Damageable")
            .and_then(|declaration| graph.declaration(declaration))
            .expect("Damageable declaration");
        let player = declarations
            .get("Player")
            .and_then(|declaration| graph.declaration(declaration))
            .expect("Player declaration");
        let diagnostics = graph.diagnostics();
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>(),
            ["unknown trait `Damageabl`", "unknown schema `Playr`"]
        );
        assert!(
            diagnostics[0]
                .labels
                .iter()
                .any(|label| label.span == damageable.span
                    && label.message == "candidate `Damageable` is declared here")
        );
        assert!(diagnostics[1].labels.iter().any(|label| {
            label.span == player.span && label.message == "candidate `Player` is declared here"
        }));
    }

    #[test]
    fn lowers_parameter_default_metadata_and_bindings() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.rewards",
            r#"
const BASE = 10

fn grant(amount = BASE, bonus = amount + 1) {
    return amount + bonus;
}
"#,
        ));
        let declarations = graph.module(module).expect("module declarations");
        let grant = declarations.get("grant").expect("grant declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let signature = graph.function_signature(grant).expect("function signature");
        assert!(signature.params[0].default_value_span.is_some());
        assert!(signature.params[1].default_value_span.is_some());
        let bindings = graph.bindings(grant).expect("function bindings");
        assert!(bindings.resolutions().any(|(_, resolution)| {
            resolution
                == &BindingResolution::Declaration(
                    declarations.get("BASE").expect("BASE declaration"),
                )
        }));
        assert!(bindings.resolutions().any(|(_, resolution)| {
            matches!(resolution, BindingResolution::Local(local) if bindings
                .local(*local)
                .is_some_and(|binding| binding.name == "amount"))
        }));
    }

    #[test]
    fn rejects_side_effecting_const_initializers() {
        let mut graph = ModuleGraph::new();
        graph.add_source(source(
            1,
            "game.config",
            r#"
const SAFE_LIMIT: int = 10 + 5;
const BAD_CALL = register_event("monster.kill");
const BAD_ASSIGN = { global_counter += 1; 0 };
fn main() { return SAFE_LIMIT; }
"#,
        ));

        let diagnostics = graph
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code.as_deref() == Some("hir::top_level_side_effect"))
            .collect::<Vec<_>>();

        assert_eq!(diagnostics.len(), 2, "{:?}", graph.diagnostics());
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("BAD_CALL"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("BAD_ASSIGN"))
        );
    }

    #[test]
    fn lowers_attribute_metadata_for_declarations_and_members() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.reward",
            r#"
#[event("monster.kill")]
pub fn grant(player: Player) {
    return null;
}

#[doc("Reward metadata")]
#[domain("gameplay")]
struct Reward {
    #[doc("Reward item id")]
    item_id: string,
}

enum QuestProgress {
    #[terminal]
    Finished { #[doc("Quest id")] quest_id: string },
}

trait Damageable {
    #[doc("Apply damage")]
    fn damage(self, amount: int) -> int;
}
"#,
        ));
        let declarations = graph.module(module).expect("module declarations");
        let grant = declarations.get("grant").expect("grant declaration");
        let reward = declarations.get("Reward").expect("Reward declaration");
        let progress = declarations
            .get("QuestProgress")
            .expect("QuestProgress declaration");
        let damageable = declarations
            .get("Damageable")
            .expect("Damageable declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let grant_attrs = graph.declaration_attrs(grant);
        assert_eq!(grant_attrs[0].name, "event");
        assert_eq!(grant_attrs[0].value.as_deref(), Some("monster.kill"));

        let reward_attrs = graph.declaration_attrs(reward);
        assert_eq!(reward_attrs[0].name, "doc");
        assert_eq!(reward_attrs[0].value.as_deref(), Some("Reward metadata"));
        assert_eq!(reward_attrs[1].name, "domain");
        let reward_shape = graph.struct_shape(reward).expect("Reward shape");
        assert_eq!(reward_shape.fields[0].attrs[0].name, "doc");
        assert_eq!(
            reward_shape.fields[0].attrs[0].value.as_deref(),
            Some("Reward item id")
        );

        let progress_shape = graph.enum_shape(progress).expect("Progress shape");
        assert_eq!(progress_shape.variants[0].attrs[0].name, "terminal");
        let crate::EnumVariantFieldsHint::Record(fields) = &progress_shape.variants[0].fields
        else {
            panic!("expected record variant fields");
        };
        assert_eq!(fields[0].attrs[0].name, "doc");
        assert_eq!(fields[0].attrs[0].value.as_deref(), Some("Quest id"));

        let trait_shape = graph.trait_shape(damageable).expect("Damageable shape");
        assert_eq!(trait_shape.methods[0].attrs[0].name, "doc");
        assert_eq!(
            trait_shape.methods[0].attrs[0].value.as_deref(),
            Some("Apply damage")
        );
    }

    #[test]
    fn lowers_enum_shape_metadata() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.quest",
            r#"
enum QuestProgress {
    None,
    Active { quest_id: string, count: int },
    Finished(quest_id: string),
}
"#,
        ));
        let declarations = graph.module(module).expect("module declarations");
        let progress = declarations
            .get("QuestProgress")
            .expect("QuestProgress declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let shape = graph.enum_shape(progress).expect("enum shape");
        let variants = shape
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(variants, ["None", "Active", "Finished"]);
        let active = shape
            .variants
            .iter()
            .find(|variant| variant.name == "Active")
            .expect("Active variant");
        let crate::EnumVariantFieldsHint::Record(fields) = &active.fields else {
            panic!("expected record fields");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["quest_id", "count"]
        );
        let finished = shape
            .variants
            .iter()
            .find(|variant| variant.name == "Finished")
            .expect("Finished variant");
        let crate::EnumVariantFieldsHint::Tuple(fields) = &finished.fields else {
            panic!("expected tuple fields");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["quest_id"]
        );
    }

    #[test]
    fn lowers_impl_metadata_and_method_bindings() {
        let mut graph = ModuleGraph::new();
        let module = graph.add_source(source(
            1,
            "game.combat",
            r#"
trait Damageable {
    fn damage(self, amount: int) -> int;
    fn alive(self) -> bool { return true; }
}
struct Player { hp: int }

impl Damageable for Player {
    fn damage(self, amount: int) -> int {
        let remaining: int = self.hp - amount;
        return remaining;
    }
}
"#,
        ));
        let declarations = graph.module(module).expect("module declarations");
        let trait_decl = declarations
            .get("Damageable")
            .expect("Damageable declaration");
        let impl_decl = declarations
            .get("impl Damageable for Player")
            .expect("impl declaration");

        assert!(graph.diagnostics().is_empty(), "{:?}", graph.diagnostics());
        let trait_shape = graph.trait_shape(trait_decl).expect("trait shape");
        assert_eq!(trait_shape.methods.len(), 2);
        assert_eq!(trait_shape.methods[0].name, "damage");
        assert!(!trait_shape.methods[0].has_default);
        assert_eq!(trait_shape.methods[1].name, "alive");
        assert!(trait_shape.methods[1].has_default);
        let default_node = trait_shape.methods[1]
            .default_body_node
            .expect("alive default body node");
        assert!(trait_shape.methods[1].default_body_span.is_some());
        let default_bindings = graph
            .trait_default_method_bindings(default_node)
            .expect("trait default method bindings");
        assert_eq!(default_bindings.locals_named("self").len(), 1);
        assert_eq!(
            graph.declaration(impl_decl).map(|decl| decl.kind),
            Some(DeclarationKind::Impl)
        );

        let metadata = graph.impl_metadata(impl_decl).expect("impl metadata");
        assert_eq!(metadata.trait_path, ["Damageable"]);
        assert_eq!(metadata.target_path, ["Player"]);
        assert_eq!(metadata.methods.len(), 1);
        let method = &metadata.methods[0];
        assert_eq!(method.name, "damage");
        assert_eq!(
            method.signature.params[1]
                .type_hint
                .as_ref()
                .map(HirTypeHint::display)
                .as_deref(),
            Some("int")
        );
        assert_eq!(
            method
                .signature
                .return_type
                .as_ref()
                .map(HirTypeHint::display)
                .as_deref(),
            Some("int")
        );

        let bindings = graph
            .impl_method_bindings(method.node)
            .expect("impl method bindings");
        let [remaining] = bindings.locals_named("remaining") else {
            panic!("expected remaining binding");
        };
        assert_eq!(
            bindings
                .local(*remaining)
                .and_then(|local| local.type_hint.as_ref())
                .map(HirTypeHint::display)
                .as_deref(),
            Some("int")
        );
        assert!(bindings.expression_count() >= 3);
    }
}
