use std::collections::BTreeMap;

use vela_common::Span;
use vela_reflect::modules::{ModuleDesc, ModuleExportDesc, ModuleExportKind};

use crate::error::{HotReloadError, HotReloadErrorKind, HotReloadResult};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleAbi {
    pub name: String,
    pub exports: Vec<ModuleExportAbi>,
    pub source_span: Option<Span>,
}

impl ModuleAbi {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            exports: Vec::new(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn from_module(module: &ModuleDesc) -> Self {
        let mut abi = Self::new(module.name.clone());
        for export in &module.exports {
            abi = abi.export(ModuleExportAbi::from_export(export));
        }
        if let Some(source_span) = module.source_span {
            abi = abi.source_span(source_span);
        }
        abi
    }

    #[must_use]
    pub fn export(mut self, export: ModuleExportAbi) -> Self {
        self.exports.push(export);
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }

    pub(crate) fn ensure_compatible(&self, next: &Self) -> HotReloadResult<()> {
        let next_exports = next
            .exports
            .iter()
            .map(|export| (export.name.as_str(), export))
            .collect::<BTreeMap<_, _>>();
        let changed_existing = self.exports.iter().any(|old| {
            if let Some(new) = next_exports.get(old.name.as_str()) {
                return *new != old;
            }
            !next.exports.iter().any(|new| old.is_compatible_rename(new))
        });
        if changed_existing {
            return Err(HotReloadError::new(HotReloadErrorKind::ChangedModuleAbi {
                module: self.name.clone(),
                old: self.exports.clone(),
                new: next.exports.clone(),
                source_span: next.source_span.map(Box::new),
            }));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleExportAbi {
    pub name: String,
    pub kind: ModuleExportKindAbi,
    pub function: Option<u128>,
}

impl ModuleExportAbi {
    #[must_use]
    pub fn function(name: impl Into<String>, function: u128) -> Self {
        Self {
            name: name.into(),
            kind: ModuleExportKindAbi::Function,
            function: Some(function),
        }
    }

    #[must_use]
    pub fn from_export(export: &ModuleExportDesc) -> Self {
        Self {
            name: export.name.clone(),
            kind: ModuleExportKindAbi::from_export_kind(export.kind),
            function: export.function.map(|function| function.get()),
        }
    }

    fn is_compatible_rename(&self, next: &Self) -> bool {
        self.kind == next.kind && self.function.is_some() && self.function == next.function
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleExportKindAbi {
    Function,
}

impl ModuleExportKindAbi {
    #[must_use]
    pub const fn from_export_kind(kind: ModuleExportKind) -> Self {
        match kind {
            ModuleExportKind::Function => Self::Function,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
        }
    }
}
