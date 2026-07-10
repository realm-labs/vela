use std::collections::BTreeMap;

use vela_def::FunctionId;
use vela_hir::body::HirBodyOwner;
use vela_hir::ids::HirBodyId;
use vela_mir::{CompileLambdaParameterTarget, CompileLambdaTarget, MirBuildError, MirSourceOrigin};

use super::{GenerationBuilder, input_error, registry_input_error};
use crate::compiler::error::CompileResult;

impl GenerationBuilder<'_, '_> {
    pub(super) fn insert_lambda_targets(&mut self) -> CompileResult<()> {
        for (function, root) in self.selected_executable_roots()? {
            self.insert_root_lambda_targets(function, root)?;
        }
        Ok(())
    }

    fn insert_root_lambda_targets(
        &mut self,
        function: FunctionId,
        root: HirBodyId,
    ) -> CompileResult<()> {
        let root_body = self
            .request
            .graph
            .body(root)
            .ok_or_else(registry_input_error)?;
        let root_origin = MirSourceOrigin::body(root, root_body.origin.span);
        let root_symbol = self
            .function_code_symbols
            .get(&function)
            .cloned()
            .ok_or_else(|| {
                input_error(MirBuildError::InconsistentInput {
                    origin: root_origin,
                    message: format!(
                        "missing code symbol for executable root #{}",
                        function.get()
                    ),
                })
            })?;
        let mut lambdas = self
            .request
            .graph
            .bodies()
            .filter(|body| matches!(body.owner, HirBodyOwner::Lambda { .. }))
            .filter(|body| {
                self.request
                    .graph
                    .body_and_ancestors(body.id)
                    .any(|ancestor| ancestor.id == root)
            })
            .map(|body| {
                let depth = self
                    .request
                    .graph
                    .body_and_ancestors(body.id)
                    .filter(|ancestor| matches!(ancestor.owner, HirBodyOwner::Lambda { .. }))
                    .count();
                (depth, body.origin.span, body.id)
            })
            .collect::<Vec<_>>();
        lambdas.sort_unstable_by_key(|(depth, span, body)| {
            (*depth, span.source, span.start, span.end, *body)
        });

        let mut symbols = BTreeMap::from([(root, root_symbol)]);
        for (_, _, body) in lambdas {
            let target = self.lambda_target(function, root, body, &symbols)?;
            for parameter in &target.parameters {
                if let Some(contract) = &parameter.contract {
                    self.remember_contract(contract, parameter.origin);
                }
            }
            symbols.insert(body, target.code_symbol.clone());
            self.targets
                .insert_lambda(function, target)
                .map_err(input_error)?;
        }
        Ok(())
    }

    fn lambda_target(
        &self,
        function: FunctionId,
        root: HirBodyId,
        body: HirBodyId,
        symbols: &BTreeMap<HirBodyId, String>,
    ) -> CompileResult<CompileLambdaTarget> {
        let hir_body = self
            .request
            .graph
            .body(body)
            .ok_or_else(registry_input_error)?;
        let HirBodyOwner::Lambda {
            parent: hir_parent,
            expression,
        } = hir_body.owner
        else {
            return Err(registry_input_error());
        };
        let origin = MirSourceOrigin::body(body, hir_body.origin.span);
        let parent = self
            .request
            .graph
            .body_and_ancestors(hir_parent)
            .find(|candidate| {
                candidate.id == root || matches!(candidate.owner, HirBodyOwner::Lambda { .. })
            })
            .map(|candidate| candidate.id)
            .ok_or_else(|| {
                input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: format!(
                        "lambda HIR body {body:?} has no runtime parent under root #{}",
                        function.get()
                    ),
                })
            })?;
        let parent_symbol = symbols.get(&parent).ok_or_else(|| {
            input_error(MirBuildError::InconsistentInput {
                origin,
                message: format!("lambda HIR body {body:?} precedes its runtime parent {parent:?}"),
            })
        })?;
        let bindings = self
            .request
            .graph
            .bindings_for_body(body)
            .ok_or_else(registry_input_error)?;
        let module = self
            .request
            .graph
            .declaration(bindings.declaration)
            .map(|declaration| declaration.module)
            .ok_or_else(registry_input_error)?;
        let parameters = hir_body
            .params
            .iter()
            .map(|parameter| {
                let binding = bindings
                    .local(parameter.local)
                    .ok_or_else(registry_input_error)?;
                let contract = binding
                    .type_hint
                    .as_ref()
                    .and_then(|hint| self.type_contract_for_hint(module, hint))
                    .and_then(super::schema::meaningful_contract);
                Ok(CompileLambdaParameterTarget {
                    parameter: parameter.id,
                    local: parameter.local,
                    name: binding.name.clone(),
                    contract,
                    origin: MirSourceOrigin::body(body, parameter.origin.span),
                })
            })
            .collect::<CompileResult<Vec<_>>>()?;
        Ok(CompileLambdaTarget {
            body,
            parent,
            expression,
            code_symbol: format!("{parent_symbol}::<lambda@{}>", hir_body.origin.span.start),
            parameters,
            origin,
        })
    }
}
