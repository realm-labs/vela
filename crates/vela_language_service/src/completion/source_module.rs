use vela_analysis::{completion::CompletionItem as AnalysisCompletionItem, type_fact::TypeFact};
use vela_hir::module_graph::ModuleGraph;
use vela_package::ModuleKey;

use crate::TextRange;

use super::{
    CompletionItem, analysis_item::dedupe_and_filter_analysis_items, label_segment_matches,
};

pub(super) fn source_module_completion_items(
    graph: &ModuleGraph,
    current_module: Option<&ModuleKey>,
    replace_range: TextRange,
    prefix: &str,
) -> Vec<CompletionItem> {
    dedupe_and_filter_analysis_items(
        module_labels(graph, current_module)
            .into_iter()
            .map(|label| AnalysisCompletionItem {
                label: label.clone(),
                kind: vela_analysis::completion::CompletionKind::Module,
                fact: TypeFact::module(label),
            })
            .collect(),
        replace_range,
        prefix,
        None,
        |item| label_segment_matches(&item.label, prefix),
    )
}

fn module_labels(graph: &ModuleGraph, current_module: Option<&ModuleKey>) -> Vec<String> {
    let Some(current_module) = current_module else {
        return graph.module_completion_labels();
    };
    let mut labels = graph.module_completion_labels_for(&current_module.package);
    labels.push("crate".to_owned());
    labels.extend(
        graph
            .dependency_aliases(&current_module.package)
            .into_iter()
            .map(|alias| alias.as_str().to_owned()),
    );
    labels
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use vela_common::SourceId;
    use vela_hir::module_graph::{ModuleGraph, ModuleSource};
    use vela_package::{ModuleKey, ModulePath, PackageAlias, PackageId};

    use super::module_labels;

    #[test]
    fn lsp_completion_lists_crate_and_dependency_aliases() {
        let app = PackageId::new("dev.vela.app").expect("app package");
        let library = PackageId::new("dev.vela.library").expect("library package");
        let alias = PackageAlias::new("shared").expect("dependency alias");
        let mut graph = ModuleGraph::with_package_dependencies(BTreeMap::from([(
            app.clone(),
            BTreeMap::from([(alias, library.clone())]),
        )]));
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            app.clone(),
            ModulePath::from_qualified("main"),
            "pub fn main() {}",
        ));
        graph.add_source(ModuleSource::new(
            SourceId::new(2),
            library,
            ModulePath::from_qualified("api"),
            "pub fn value() {}",
        ));

        let labels = module_labels(
            &graph,
            Some(&ModuleKey::new(app, ModulePath::from_qualified("main"))),
        );
        assert!(labels.contains(&"crate".to_owned()));
        assert!(labels.contains(&"shared".to_owned()));
        assert!(!labels.contains(&"api".to_owned()));
    }
}
