use vela_common::{Diagnostic, Span};

use super::SyntaxBindingLowerer;
use crate::binding::BindingResolution;
use crate::binding::name_candidates::{NameCandidate, closest_name_candidate};
use crate::ids::HirDeclId;

impl SyntaxBindingLowerer<'_> {
    pub(super) fn resolve_constructor_path(&self, path: &[String]) -> Option<BindingResolution> {
        if let [name] = path {
            return self.resolve_declaration_name(name);
        }
        if let Some(name) = path.first()
            && let Some(resolution) = self.resolve_declaration_name(name)
        {
            return Some(resolution);
        }
        if let Some(declaration) = self.qualified_declaration(path) {
            return Some(BindingResolution::Declaration(declaration));
        }
        let (_, enum_path) = path.split_last()?;
        self.qualified_declaration(enum_path)
            .map(BindingResolution::Declaration)
    }

    pub(super) fn resolve_name(&self, name: &str) -> Option<BindingResolution> {
        for scope in self.scopes.iter().rev() {
            if let Some(local) = scope.locals.get(name) {
                return Some(BindingResolution::Local(*local));
            }
        }
        self.resolve_declaration_name(name)
    }

    fn resolve_declaration_name(&self, name: &str) -> Option<BindingResolution> {
        if let Some((_, declaration)) = self
            .module_declarations
            .iter()
            .find(|(declaration_name, _)| declaration_name == name)
        {
            return Some(BindingResolution::Declaration(*declaration));
        }
        self.imports.iter().find_map(|import| {
            if import.name != name {
                return None;
            }
            Some(match import.declaration {
                Some(declaration) => BindingResolution::Declaration(declaration),
                None => BindingResolution::Import(import.name.clone()),
            })
        })
    }

    pub(super) fn resolve_declaration_path(&self, path: &[String]) -> Option<BindingResolution> {
        let [name] = path else {
            if let Some(declaration) = self.qualified_declaration(path) {
                return Some(BindingResolution::Declaration(declaration));
            }
            return Some(BindingResolution::QualifiedPath(path.to_vec()));
        };
        self.resolve_declaration_name(name)
    }

    fn qualified_declaration(&self, path: &[String]) -> Option<HirDeclId> {
        self.qualified_declarations
            .iter()
            .find_map(|(declaration_path, declaration)| {
                (declaration_path == path).then_some(*declaration)
            })
    }

    pub(super) fn unresolved_name_diagnostic(&self, name: &str, span: Span) -> Diagnostic {
        let mut diagnostic = Diagnostic::error(format!("unresolved name `{name}`"))
            .with_code("hir::unresolved_name")
            .with_span(span);
        let Some(candidate) = self.name_candidate(name) else {
            return diagnostic.with_label(span, "no similar names found");
        };
        diagnostic = diagnostic.with_label(span, format!("did you mean `{}`?", candidate.name));
        if let Some(candidate_span) = candidate.span
            && candidate_span != span
        {
            diagnostic = diagnostic.with_label(
                candidate_span,
                format!("candidate `{}` is declared here", candidate.name),
            );
        }
        diagnostic
    }

    fn name_candidate(&self, name: &str) -> Option<NameCandidate> {
        let mut candidates = self
            .scopes
            .iter()
            .rev()
            .flat_map(|scope| {
                scope.locals.iter().filter_map(|(name, local)| {
                    self.locals
                        .get(local)
                        .map(|binding| NameCandidate::new(name.clone(), Some(binding.span)))
                })
            })
            .chain(
                self.module_declarations
                    .iter()
                    .map(|(name, _)| NameCandidate::new(name.clone(), None)),
            )
            .chain(
                self.imports
                    .iter()
                    .map(|import| NameCandidate::new(import.name.clone(), None)),
            )
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.name.cmp(&right.name));
        candidates.dedup_by(|left, right| left.name == right.name);
        closest_name_candidate(name, candidates)
    }
}
