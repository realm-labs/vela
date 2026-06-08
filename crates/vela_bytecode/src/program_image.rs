use std::collections::BTreeMap;

use vela_common::GlobalSlot;
use vela_hir::module_graph::ModuleGraph;

use crate::script_methods::ScriptMethodTable;
use crate::{CodeObject, FunctionIndex, InstructionKind, Program, ProgramCode};

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
        Self::from_parts(
            program.functions.values().cloned(),
            program.global_names().iter().cloned(),
            program.script_methods().clone(),
            program.script_metadata().cloned(),
        )
    }

    #[must_use]
    pub fn from_parts(
        functions: impl IntoIterator<Item = CodeObject>,
        global_names: impl IntoIterator<Item = String>,
        script_methods: ScriptMethodTable,
        script_metadata: Option<ModuleGraph>,
    ) -> Self {
        let functions = functions.into_iter();
        let mut indexed_functions = Vec::with_capacity(functions.size_hint().0);
        let mut function_by_name = BTreeMap::new();
        for function in functions {
            let name = function.name.clone();
            let function = flatten_function(function, &mut indexed_functions);
            let index = FunctionIndex(indexed_functions.len());
            function_by_name.insert(name, index);
            indexed_functions.push(function);
        }

        let global_names = global_names.into_iter().collect::<Vec<_>>();
        let global_slots = global_names
            .iter()
            .enumerate()
            .map(|(slot, name)| (name.clone(), GlobalSlot::new(slot)))
            .collect();

        Self {
            functions: indexed_functions,
            function_by_name,
            global_names,
            global_slots,
            script_methods,
            script_metadata,
        }
    }

    #[must_use]
    pub fn to_program(&self) -> Program {
        let mut program = Program::new();
        for index in self.function_by_name.values().copied() {
            if let Some(function) = self.function_for_program(index, &mut Vec::new()) {
                program.insert_function(function);
            }
        }
        program.set_global_layout(self.global_names.clone());
        program.set_script_methods(self.script_methods.clone());
        if let Some(graph) = &self.script_metadata {
            program.set_script_metadata(graph.clone());
        }
        program
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

    pub fn verify(&self) -> Result<(), crate::verification::VerificationError> {
        crate::verification::verify_program_image(self)
    }

    fn function_for_program(
        &self,
        index: FunctionIndex,
        stack: &mut Vec<FunctionIndex>,
    ) -> Option<CodeObject> {
        if stack.contains(&index) {
            return None;
        }
        let mut function = self.function(index)?.clone();
        stack.push(index);
        let mut nested_functions = Vec::new();
        for instruction in &mut function.instructions {
            if let InstructionKind::MakeClosure {
                function: target, ..
            } = &mut instruction.kind
                && let Some(nested) = self.function_for_program(*target, stack)
            {
                *target = FunctionIndex(nested_functions.len());
                nested_functions.push(nested);
            }
        }
        function.nested_functions = nested_functions;
        stack.pop();
        Some(function)
    }
}

impl ProgramCode for ProgramImage {
    fn function(&self, name: &str) -> Option<&CodeObject> {
        self.function_by_name(name)
    }

    fn function_by_index(&self, index: FunctionIndex) -> Option<&CodeObject> {
        self.function(index)
    }

    fn script_method(&self, type_name: &str, method: &str) -> Option<&CodeObject> {
        let method = self.script_methods.get(type_name, method)?;
        self.function_by_name(&method.function)
    }

    fn script_method_id(&self, type_name: &str, method: &str) -> Option<vela_common::MethodId> {
        self.script_methods
            .get(type_name, method)
            .map(|method| method.id)
    }

    fn script_method_by_id(
        &self,
        type_name: &str,
        method_id: vela_common::MethodId,
    ) -> Option<&CodeObject> {
        let method = self.script_methods.get_by_id(type_name, method_id)?;
        self.function_by_name(&method.function)
    }
}

fn flatten_function(
    mut function: CodeObject,
    indexed_functions: &mut Vec<CodeObject>,
) -> CodeObject {
    let nested_functions = std::mem::take(&mut function.nested_functions);
    if nested_functions.is_empty() {
        return function;
    }

    let mut remapped = Vec::with_capacity(nested_functions.len());
    for nested in nested_functions {
        let nested = flatten_function(nested, indexed_functions);
        let index = FunctionIndex(indexed_functions.len());
        indexed_functions.push(nested);
        remapped.push(index);
    }

    rewrite_closure_function_indices(&mut function, &remapped);
    function
}

fn rewrite_closure_function_indices(function: &mut CodeObject, remapped: &[FunctionIndex]) {
    for instruction in &mut function.instructions {
        if let InstructionKind::MakeClosure { function, .. } = &mut instruction.kind
            && let Some(index) = remapped.get(function.0)
        {
            *function = *index;
        }
    }
}

#[cfg(test)]
mod tests {
    use vela_common::{GlobalSlot, MethodId};

    use crate::{CodeObject, Constant, Instruction, InstructionKind, Program, Register};

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

    #[test]
    fn image_flattens_nested_closure_functions() {
        let mut program = Program::new();
        let mut main = CodeObject::new("main", 1);
        let closure = CodeObject::new("main::<lambda>", 1);
        let local_function = main.push_nested_function(closure);
        main.push_instruction(Instruction::new(InstructionKind::MakeClosure {
            dst: Register(0),
            function: local_function,
            captures: Vec::new(),
        }));
        program.insert_function(main);

        let image = ProgramImage::from_program(&program);
        let main_index = image.function_index("main").expect("main function index");
        let main = image.function(main_index).expect("main function");
        let closure_index = match &main.instructions[0].kind {
            InstructionKind::MakeClosure { function, .. } => *function,
            other => panic!("expected MakeClosure instruction, found {other:?}"),
        };

        assert!(main.nested_functions.is_empty());
        assert_eq!(image.function_count(), 2);
        assert_eq!(
            image
                .function(closure_index)
                .expect("image closure function")
                .name,
            "main::<lambda>"
        );
    }

    #[test]
    fn image_rebuilds_nested_closures_for_program_compatibility() {
        let mut program = Program::new();
        let mut main = CodeObject::new("main", 1);
        let closure = CodeObject::new("main::<lambda>", 1);
        let local_function = main.push_nested_function(closure);
        main.push_instruction(Instruction::new(InstructionKind::MakeClosure {
            dst: Register(0),
            function: local_function,
            captures: Vec::new(),
        }));
        program.insert_function(main);

        let rebuilt = ProgramImage::from_program(&program).to_program();
        let main = rebuilt.function("main").expect("rebuilt main function");
        let closure_index = match &main.instructions[0].kind {
            InstructionKind::MakeClosure { function, .. } => *function,
            other => panic!("expected MakeClosure instruction, found {other:?}"),
        };

        assert_eq!(rebuilt.functions.len(), 1);
        assert_eq!(
            main.nested_function(closure_index)
                .expect("rebuilt nested closure")
                .name,
            "main::<lambda>"
        );
    }
}
