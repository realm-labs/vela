use vela_analysis::logical_records::{LogicalRecordFact, LogicalRecordKind};
use vela_mir::{
    CompileFieldAccess, CompileFieldDescriptor, CompileGuardKey, CompileGuardTarget,
    CompileTypeClass, CompileTypeDescriptor, MirGuardLocation, MirSourceOrigin,
};

use super::schema::{contract_from_fact, meaningful_contract};
use super::{GenerationBuilder, input_error};
use crate::compiler::error::CompileResult;

impl GenerationBuilder<'_, '_> {
    pub(super) fn ensure_logical_record(
        &mut self,
        kind: LogicalRecordKind,
        origin: MirSourceOrigin,
    ) -> CompileResult<()> {
        if !self.inserted_logical_records.insert(kind) {
            return Ok(());
        }
        let manifest = LogicalRecordFact::manifest(kind);
        let type_id = manifest.type_id();
        self.type_names
            .insert(type_id, manifest.runtime_name().to_owned());
        self.targets
            .insert_type_descriptor(
                CompileTypeDescriptor {
                    id: type_id,
                    canonical_name: kind.canonical_name(),
                    class: CompileTypeClass::Standard,
                    shape: Some(manifest.shape()),
                    fields: manifest.fields().map(|field| field.id()).collect(),
                    variants: Vec::new(),
                },
                origin,
            )
            .map_err(input_error)?;

        for field in manifest.fields() {
            let contract = contract_from_fact(
                field.fact(),
                &self.registry_facts,
                self.request.graph,
                &self.type_ids,
                &self.type_shapes,
            )
            .and_then(meaningful_contract);
            if let Some(contract) = &contract {
                self.remember_contract(contract, origin);
            }
            self.targets
                .insert_field_descriptor(
                    CompileFieldDescriptor {
                        id: field.id(),
                        owner: type_id,
                        variant: None,
                        name: field.name().to_owned(),
                        contract: contract.clone(),
                        declaration_order: field.canonical_slot(),
                        access: CompileFieldAccess::script(),
                        host_runtime: None,
                    },
                    origin,
                )
                .map_err(input_error)?;
            if let Some(contract) = contract {
                self.insert_guard_once(
                    CompileGuardKey::Field(field.id()),
                    CompileGuardTarget::new(contract, MirGuardLocation::Field, field.name()),
                    origin,
                )?;
            }
        }
        Ok(())
    }
}
