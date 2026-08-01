use vela_def::{FieldId, FunctionId, TypeId, VariantId};
use vela_hir::body::{HirBody, HirBodyOwner};
use vela_hir::ids::HirExprId;
use vela_mir::{
    CompileTryFamily, CompileTryLayoutTarget, CompileTryTarget, MirBuildError, MirSourceOrigin,
    MirTypeContract,
};
use vela_registry::TypeKindDef;

use super::external::ExternalCatalog;
use super::{GenerationBuilder, input_error, registry_input_error};
use crate::compiler::error::{CompileError, CompileErrorKind, CompileResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TryLayouts {
    option: CompileTryLayoutTarget,
    result: CompileTryLayoutTarget,
}

impl TryLayouts {
    pub(super) fn from_catalog(catalog: &ExternalCatalog) -> CompileResult<Self> {
        let option = CompileTryLayoutTarget {
            family: CompileTryFamily::Option,
            type_id: required_type_id("Option")?,
            continue_variant: required_variant_id("Option", "Some")?,
            break_variant: required_variant_id("Option", "None")?,
            continue_payload: required_field_id("Option::Some", "0")?,
        };
        let result = CompileTryLayoutTarget {
            family: CompileTryFamily::Result,
            type_id: required_type_id("Result")?,
            continue_variant: required_variant_id("Result", "Ok")?,
            break_variant: required_variant_id("Result", "Err")?,
            continue_payload: required_field_id("Result::Ok", "0")?,
        };
        validate_layout(catalog, option, "Option")?;
        validate_layout(catalog, result, "Result")?;
        Ok(Self { option, result })
    }

    fn for_contract(self, contract: Option<&MirTypeContract>) -> CompileTryTarget {
        match contract {
            Some(MirTypeContract::Option(_)) => CompileTryTarget::Expected(self.option),
            Some(MirTypeContract::Result { .. }) => CompileTryTarget::Expected(self.result),
            Some(
                MirTypeContract::Any
                | MirTypeContract::TaskError
                | MirTypeContract::Primitive(_)
                | MirTypeContract::Tuple(_)
                | MirTypeContract::Array(_)
                | MirTypeContract::Map { .. }
                | MirTypeContract::Set(_)
                | MirTypeContract::Range
                | MirTypeContract::Iterator(_)
                | MirTypeContract::Callable { .. }
                | MirTypeContract::Definition(_)
                | MirTypeContract::Shape { .. }
                | MirTypeContract::Variant { .. }
                | MirTypeContract::Host(_),
            )
            | None => CompileTryTarget::Dynamic {
                option: self.option,
                result: self.result,
            },
        }
    }
}

impl GenerationBuilder<'_, '_> {
    pub(super) fn insert_try_target(
        &mut self,
        function: FunctionId,
        body: &HirBody,
        expression: HirExprId,
    ) -> CompileResult<()> {
        let origin = self
            .expression_origin(expression)
            .ok_or_else(registry_input_error)?;
        let contract = self
            .function_return_contracts
            .get(&function)
            .ok_or_else(|| {
                input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: format!(
                        "missing owning return contract for try expression in function #{}",
                        function.get()
                    ),
                })
            })?
            .as_ref();
        let in_lambda = self
            .request
            .graph
            .body_and_ancestors(body.id)
            .any(|ancestor| matches!(&ancestor.owner, HirBodyOwner::Lambda { .. }));
        let target = self
            .try_layouts
            .for_contract((!in_lambda).then_some(contract).flatten());
        self.ensure_try_descriptors(target, origin)?;
        self.targets
            .insert_try_target(function, expression, target, origin)
            .map_err(input_error)
    }

    fn ensure_try_descriptors(
        &mut self,
        target: CompileTryTarget,
        origin: MirSourceOrigin,
    ) -> CompileResult<()> {
        match target {
            CompileTryTarget::Expected(layout) => self.ensure_external_type(layout.type_id, origin),
            CompileTryTarget::Dynamic { option, result } => {
                self.ensure_external_type(option.type_id, origin)?;
                self.ensure_external_type(result.type_id, origin)
            }
        }
    }
}

fn validate_layout(
    catalog: &ExternalCatalog,
    layout: CompileTryLayoutTarget,
    name: &str,
) -> CompileResult<()> {
    let Some(definition) = catalog.ty(layout.type_id) else {
        return Err(snapshot_error(format!(
            "standard {name} try layout references missing type #{}",
            layout.type_id.get()
        )));
    };
    if definition.kind != TypeKindDef::ScriptEnum {
        return Err(snapshot_error(format!(
            "standard {name} try layout type #{} is not an enum",
            layout.type_id.get()
        )));
    }
    for (variant, role) in [
        (layout.continue_variant, "continue"),
        (layout.break_variant, "break"),
    ] {
        let Some(definition) = catalog.variant(variant) else {
            return Err(snapshot_error(format!(
                "standard {name} try layout references missing {role} variant #{}",
                variant.get()
            )));
        };
        if definition.owner != layout.type_id {
            return Err(snapshot_error(format!(
                "standard {name} try layout {role} variant #{} belongs to type #{}, expected #{}",
                variant.get(),
                definition.owner.get(),
                layout.type_id.get()
            )));
        }
    }
    if layout.continue_variant == layout.break_variant {
        return Err(snapshot_error(format!(
            "standard {name} try layout uses one variant for continue and break"
        )));
    }
    let Some(payload) = catalog.field(layout.continue_payload) else {
        return Err(snapshot_error(format!(
            "standard {name} try layout references missing continue payload #{}",
            layout.continue_payload.get()
        )));
    };
    if payload.owner != layout.type_id || payload.variant != Some(layout.continue_variant) {
        return Err(snapshot_error(format!(
            "standard {name} try layout continue payload #{} has mismatched ownership",
            layout.continue_payload.get()
        )));
    }
    Ok(())
}

fn required_type_id(name: &str) -> CompileResult<TypeId> {
    vela_stdlib::std_type_id(name).ok_or_else(|| {
        snapshot_error(format!(
            "standard library manifest is missing try type `{name}`"
        ))
    })
}

fn required_variant_id(owner: &str, name: &str) -> CompileResult<VariantId> {
    vela_stdlib::std_variant_id(owner, name).ok_or_else(|| {
        snapshot_error(format!(
            "standard library manifest is missing try variant `{owner}::{name}`"
        ))
    })
}

fn required_field_id(owner: &str, name: &str) -> CompileResult<FieldId> {
    vela_stdlib::std_field_id(owner, name).ok_or_else(|| {
        snapshot_error(format!(
            "standard library manifest is missing try field `{owner}::{name}`"
        ))
    })
}

fn snapshot_error(message: String) -> CompileError {
    CompileError::new(CompileErrorKind::RegistrySnapshot(message))
}

#[cfg(test)]
mod tests {
    use vela_def::{DefPath, script_type_id, script_type_path};
    use vela_registry::{Def, DefinitionRegistry, TypeDef};

    use super::TryLayouts;
    use crate::compiler::error::CompileErrorKind;
    use crate::compiler::semantic_input::external::ExternalCatalog;

    #[test]
    fn missing_standard_try_layout_is_a_registry_snapshot_error() {
        let error = TryLayouts::from_catalog(&ExternalCatalog::default())
            .expect_err("an empty catalog must not define the standard try layouts");

        assert!(matches!(
            &error.kind,
            CompileErrorKind::RegistrySnapshot(message) if message.contains("missing type")
        ));
    }

    #[test]
    fn mismatched_standard_try_layout_is_a_registry_snapshot_error() {
        let standard = vela_stdlib::standard_registry().expect("standard registry");
        let option = vela_stdlib::std_type_id("Option").expect("Option ID");
        let some = vela_stdlib::std_variant_id("Option", "Some").expect("Some ID");
        let wrong_owner = TypeDef::new(DefPath::ty("test", ["try"], "WrongOwner")).id;
        let mut registry = DefinitionRegistry::new();
        registry
            .register_type(TypeDef::new(DefPath::ty("test", ["try"], "WrongOwner")))
            .expect("wrong owner type");
        for definition in standard.compile_view().definitions() {
            let definition = match definition {
                Def::Variant(definition) if definition.id == some => {
                    let mut definition = definition.clone();
                    definition.owner = wrong_owner;
                    Def::Variant(definition)
                }
                definition => definition.clone(),
            };
            registry
                .insert(definition)
                .expect("unique standard definition");
        }
        let slots = registry
            .compile_view()
            .declaration_slots()
            .expect("registry declaration slots");
        let catalog =
            ExternalCatalog::from_view(registry.compile_view(), &slots).expect("external catalog");

        let error = TryLayouts::from_catalog(&catalog)
            .expect_err("a cross-type Some edge must be rejected");
        assert!(matches!(
            &error.kind,
            CompileErrorKind::RegistrySnapshot(message)
                if message.contains(&format!("expected #{}", option.get()))
        ));
    }

    #[test]
    fn standard_and_script_try_family_ids_keep_package_qualified_identity() {
        let standard = DefPath::ty("std", std::iter::empty::<&str>(), "Result");
        let script = script_type_path(vela_package::PackageId::anonymous().as_str(), "Result");
        let standard_id = vela_stdlib::std_type_id("Result").expect("Result ID");
        let script_id = script_type_id(
            vela_package::PackageId::anonymous().as_str(),
            "Result",
            None,
        );

        assert_eq!(standard.canonical_name(), "std::Result");
        assert_eq!(
            script.canonical_name(),
            format!("{}::Result", vela_package::PackageId::anonymous())
        );
        assert_eq!(
            crate::compiler::semantic_input::external::source_name(&standard),
            "Result"
        );
        assert_ne!(standard_id, script_id);
    }
}
