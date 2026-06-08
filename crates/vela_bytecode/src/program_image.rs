use std::collections::BTreeMap;

use vela_common::GlobalSlot;
use vela_hir::module_graph::ModuleGraph;

use crate::script_methods::ScriptMethodTable;
use crate::{CodeObject, FunctionIndex, Program};

#[derive(Clone, Debug, PartialEq)]
pub struct ProgramImage {
    functions: Vec<CodeObject>,
    function_by_name: BTreeMap<String, FunctionIndex>,
    global_names: Vec<String>,
    global_slots: BTreeMap<String, GlobalSlot>,
    script_methods: ScriptMethodTable,
    script_metadata: Option<ModuleGraph>,
}

impl ProgramImage {
    #[must_use]
    pub fn from_program(program: &Program) -> Self {
        let mut functions = Vec::with_capacity(program.functions.len());
        let mut function_by_name = BTreeMap::new();
        for function in program.functions.values() {
            let index = FunctionIndex(functions.len());
            function_by_name.insert(function.name.clone(), index);
            functions.push(function.clone());
        }

        let global_names = program.global_names().to_vec();
        let global_slots = global_names
            .iter()
            .enumerate()
            .map(|(slot, name)| (name.clone(), GlobalSlot::new(slot)))
            .collect();

        Self {
            functions,
            function_by_name,
            global_names,
            global_slots,
            script_methods: program.script_methods().clone(),
            script_metadata: program.script_metadata().cloned(),
        }
    }

    #[must_use]
    pub fn function(&self, index: FunctionIndex) -> Option<&CodeObject> {
        self.functions.get(index.0)
    }

    #[must_use]
    pub fn function_by_name(&self, name: &str) -> Option<&CodeObject> {
        self.function(self.function_index(name)?)
    }

    #[must_use]
    pub fn function_index(&self, name: &str) -> Option<FunctionIndex> {
        self.function_by_name.get(name).copied()
    }

    pub fn functions(&self) -> impl Iterator<Item = (FunctionIndex, &CodeObject)> {
        self.functions
            .iter()
            .enumerate()
            .map(|(index, function)| (FunctionIndex(index), function))
    }

    #[must_use]
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    #[must_use]
    pub fn global_slot(&self, name: &str) -> Option<GlobalSlot> {
        self.global_slots.get(name).copied()
    }

    #[must_use]
    pub fn global_name(&self, slot: GlobalSlot) -> Option<&str> {
        self.global_names.get(slot.get()).map(String::as_str)
    }

    #[must_use]
    pub fn global_names(&self) -> &[String] {
        &self.global_names
    }

    #[must_use]
    pub fn script_methods(&self) -> &ScriptMethodTable {
        &self.script_methods
    }

    #[must_use]
    pub fn script_metadata(&self) -> Option<&ModuleGraph> {
        self.script_metadata.as_ref()
    }

    #[must_use]
    pub fn cache_site_count(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.cache_sites.len())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use vela_common::{GlobalSlot, MethodId};

    use crate::{CodeObject, Constant, Program};

    use super::ProgramImage;

    #[test]
    fn image_indexes_functions_by_stable_names() {
        let mut program = Program::new();
        program.insert_function(CodeObject::new("zeta", 0));
        program.insert_function(CodeObject::new("alpha", 0));

        let image = ProgramImage::from_program(&program);
        let alpha = image
            .function_index("alpha")
            .expect("alpha should have index");
        let zeta = image
            .function_index("zeta")
            .expect("zeta should have index");

        assert_ne!(alpha, zeta);
        assert_eq!(image.function(alpha).expect("alpha function").name, "alpha");
        assert_eq!(
            image.function_by_name("zeta").expect("zeta function").name,
            "zeta"
        );
        assert_eq!(image.function_count(), 2);
    }

    #[test]
    fn image_preserves_global_layout_and_script_methods() {
        let mut program = Program::new();
        program.set_global_layout(["main::first".to_owned(), "main::second".to_owned()]);
        program.insert_function(CodeObject::new("main", 0));
        program.insert_script_method("Player", "bonus", MethodId::new(7), "main");

        let image = ProgramImage::from_program(&program);

        assert_eq!(image.global_slot("main::first"), Some(GlobalSlot::new(0)));
        assert_eq!(image.global_name(GlobalSlot::new(1)), Some("main::second"));
        assert_eq!(image.global_names(), program.global_names());
        assert_eq!(
            image
                .script_methods()
                .get_by_id("Player", MethodId::new(7))
                .map(|method| method.function.as_str()),
            Some("main")
        );
    }

    #[test]
    fn image_is_detached_from_later_program_mutation() {
        let mut program = Program::new();
        let mut main = CodeObject::new("main", 0);
        main.push_constant(Constant::Int(1));
        program.insert_function(main);

        let image = ProgramImage::from_program(&program);
        program
            .functions
            .get_mut("main")
            .expect("main function")
            .push_constant(Constant::Int(2));

        assert_eq!(
            image
                .function_by_name("main")
                .expect("image main")
                .constants
                .len(),
            1
        );
    }
}
