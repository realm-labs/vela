use std::collections::{BTreeMap, btree_map::Entry};

mod schema_diagnostics;

use vela_common::{Diagnostic, SourceId, Span};
use vela_syntax::ast::{
    Block, EnumItem, EnumVariantFields, FunctionItem, ImplItem, ItemKind, Param, SourceFile,
    StructItem, TraitItem, TraitMethod, Visibility,
};
use vela_syntax::parser::parse_source;

use crate::attributes::{HirAttribute, attrs_from_syntax};
use crate::binding::{BindingMap, FunctionBindingInput, ImportBinding, bind_function};
use crate::ids::{HirDeclId, HirNodeId, ModuleId};
use crate::top_level::validate_const_initializer;
use crate::type_hint::{
    ConstMetadata, EnumShape, FunctionSignature, HirTypeHint, ImplMetadata, ParamHint,
    StructFieldHint, StructShape, TraitShape,
};

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
                    self.validate_struct_shape(record);
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
                    self.validate_enum_shape(enumeration);
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
                    self.validate_trait_shape(trait_item);
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
                    self.validate_impl_shape(impl_item);
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

        self.validate_import_bindings(&hir_module);

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

    fn validate_import_bindings(&mut self, module: &HirModule) {
        let mut imported_names = BTreeMap::new();
        for import in &module.imports {
            let Some(name) = import_binding_name(import) else {
                continue;
            };
            if let Some(previous_span) = imported_names.insert(name.clone(), import.span) {
                self.diagnostics.push(
                    Diagnostic::error(format!("duplicate import `{name}`"))
                        .with_code("hir::duplicate_import")
                        .with_span(import.span)
                        .with_label(previous_span, "previous import is here")
                        .with_label(import.span, "duplicate import is here"),
                );
            }
            if let Some(declaration) = module
                .declarations
                .get(&name)
                .and_then(|declaration| self.declarations.get(&declaration))
            {
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "import `{name}` conflicts with a local declaration"
                    ))
                    .with_code("hir::import_conflict")
                    .with_span(import.span)
                    .with_label(declaration.span, "local declaration is here")
                    .with_label(import.span, "conflicting import is here"),
                );
            }
        }
    }

    fn validate_struct_shape(&mut self, item: &StructItem) {
        self.validate_member_names(
            &item.fields,
            |field| (&field.name, field.span),
            "field",
            "hir::duplicate_field",
        );
    }

    fn validate_enum_shape(&mut self, item: &EnumItem) {
        self.validate_member_names(
            &item.variants,
            |variant| (&variant.name, variant.span),
            "variant",
            "hir::duplicate_variant",
        );
        for variant in &item.variants {
            match &variant.fields {
                EnumVariantFields::Unit => {}
                EnumVariantFields::Tuple(params) => {
                    self.validate_member_names(
                        params,
                        |param| (&param.name, param.span),
                        "variant field",
                        "hir::duplicate_variant_field",
                    );
                }
                EnumVariantFields::Record(fields) => {
                    self.validate_member_names(
                        fields,
                        |field| (&field.name, field.span),
                        "variant field",
                        "hir::duplicate_variant_field",
                    );
                }
            }
        }
    }

    fn validate_trait_shape(&mut self, item: &TraitItem) {
        self.validate_member_names(
            &item.methods,
            |method| (&method.name, method.span),
            "trait method",
            "hir::duplicate_trait_method",
        );
        for method in &item.methods {
            self.validate_member_names(
                &method.params,
                |param| (&param.name, param.span),
                "parameter",
                "hir::duplicate_parameter",
            );
        }
    }

    fn validate_impl_shape(&mut self, item: &ImplItem) {
        self.validate_member_names(
            &item.methods,
            |method| (&method.function.name, method.function.body.span),
            "impl method",
            "hir::duplicate_impl_method",
        );
    }

    fn validate_member_names<T>(
        &mut self,
        members: &[T],
        member_name: impl Fn(&T) -> (&String, Span),
        label: &str,
        code: &'static str,
    ) {
        let mut names = BTreeMap::new();
        for member in members {
            let (name, span) = member_name(member);
            if let Some(previous_span) = names.insert(name.clone(), span) {
                self.diagnostics.push(
                    Diagnostic::error(format!("duplicate {label} `{name}`"))
                        .with_code(code)
                        .with_span(span)
                        .with_label(previous_span, format!("previous {label} is here"))
                        .with_label(span, format!("duplicate {label} is here")),
                );
            }
        }
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
mod tests;
