use std::collections::BTreeMap;

use vela_common::GlobalSlot;
use vela_hir::module_graph::ModuleGraph;

use crate::script_methods::ScriptMethodTable;
use crate::{
    CacheSiteDesc, CacheSiteId, CacheSiteLayout, CodeObject, FunctionIndex, InstructionKind,
    Program, ProgramCode,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ProgramImage {
    functions: Box<[CodeObject]>,
    function_by_name: BTreeMap<String, FunctionIndex>,
    global_names: Box<[String]>,
    global_slots: BTreeMap<String, GlobalSlot>,
    cache_sites: Box<[CacheSiteDesc]>,
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
        let cache_sites = rewrite_image_cache_sites(&mut indexed_functions);

        let global_names = global_names.into_iter().collect::<Vec<_>>();
        let global_slots = global_names
            .iter()
            .enumerate()
            .map(|(slot, name)| (name.clone(), GlobalSlot::new(slot)))
            .collect();

        Self {
            functions: indexed_functions.into_boxed_slice(),
            function_by_name,
            global_names: global_names.into_boxed_slice(),
            global_slots,
            cache_sites,
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
        program.set_global_layout(self.global_names.iter().cloned());
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
        self.cache_sites.len()
    }

    #[must_use]
    pub fn cache_site(&self, site: CacheSiteId) -> Option<&CacheSiteDesc> {
        self.cache_sites.get(site.index())
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
        localize_function_cache_sites(&mut function);
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

fn rewrite_image_cache_sites(functions: &mut [CodeObject]) -> Box<[CacheSiteDesc]> {
    let mut image_sites = Vec::new();
    for function in functions {
        let local_sites = function.cache_sites.sites().to_vec();
        if local_sites.is_empty() {
            continue;
        }

        let mut remapped = vec![None; local_sites.len()];
        let mut function_sites = Vec::with_capacity(local_sites.len());
        for site in local_sites {
            let id = CacheSiteId::new(
                u32::try_from(image_sites.len()).expect("cache site count exceeds u32::MAX"),
            );
            if let Some(slot) = remapped.get_mut(site.id.index()) {
                *slot = Some(id);
            }
            let site = CacheSiteDesc::new(id, site.kind, site.function, site.instruction_offset);
            image_sites.push(site.clone());
            function_sites.push(site);
        }

        rewrite_instruction_cache_sites(function, &remapped);
        function.cache_sites = CacheSiteLayout::new(function_sites);
    }
    image_sites.into_boxed_slice()
}

fn rewrite_instruction_cache_sites(function: &mut CodeObject, remapped: &[Option<CacheSiteId>]) {
    for instruction in &mut function.instructions {
        if let InstructionKind::LoadGlobal {
            cache_site: Some(site),
            ..
        } = &mut instruction.kind
            && let Some(Some(id)) = remapped.get(site.index())
        {
            *site = *id;
        }
    }
}

fn localize_function_cache_sites(function: &mut CodeObject) {
    let image_sites = function.cache_sites.sites().to_vec();
    if image_sites.is_empty() {
        return;
    }

    let mut remapped = BTreeMap::new();
    let mut local_sites = Vec::with_capacity(image_sites.len());
    for (index, mut site) in image_sites.into_iter().enumerate() {
        let local_id =
            CacheSiteId::new(u32::try_from(index).expect("cache site count exceeds u32::MAX"));
        remapped.insert(site.id, local_id);
        site.id = local_id;
        local_sites.push(site);
    }

    for instruction in &mut function.instructions {
        if let InstructionKind::LoadGlobal {
            cache_site: Some(site),
            ..
        } = &mut instruction.kind
            && let Some(local_id) = remapped.get(site)
        {
            *site = *local_id;
        }
    }

    function.cache_sites = CacheSiteLayout::new(local_sites);
}

#[cfg(test)]
mod tests {
    use vela_common::{GlobalSlot, MethodId};

    use crate::{
        CacheSiteId, CacheSiteKind, CodeObject, Constant, Instruction, InstructionKind,
        InstructionOffset, Program, Register,
    };

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

    #[test]
    fn image_rewrites_cache_site_ids_to_image_global_indexes() {
        let mut first = CodeObject::new("read_first", 1);
        let first_local = first.push_cache_site(CacheSiteKind::GlobalRead, InstructionOffset(0));
        first.push_instruction(Instruction::new(InstructionKind::LoadGlobal {
            dst: Register(0),
            global: "main::first".to_owned(),
            slot: None,
            cache_site: Some(first_local),
        }));
        let mut second = CodeObject::new("read_second", 1);
        let second_local = second.push_cache_site(CacheSiteKind::GlobalRead, InstructionOffset(0));
        second.push_instruction(Instruction::new(InstructionKind::LoadGlobal {
            dst: Register(0),
            global: "main::second".to_owned(),
            slot: None,
            cache_site: Some(second_local),
        }));
        assert_eq!(first_local, second_local);

        let mut program = Program::new();
        program.insert_function(first);
        program.insert_function(second);
        let image = ProgramImage::from_program(&program);
        let first = image
            .function_by_name("read_first")
            .expect("first function");
        let second = image
            .function_by_name("read_second")
            .expect("second function");
        let first_site = load_global_cache_site(first);
        let second_site = load_global_cache_site(second);

        assert_eq!(image.cache_site_count(), 2);
        assert_ne!(first_site, second_site);
        assert_eq!(
            image.cache_site(first_site).expect("first site").id,
            first_site
        );
        assert_eq!(
            image.cache_site(second_site).expect("second site").id,
            second_site
        );

        let rebuilt = image.to_program();
        assert_eq!(
            load_global_cache_site(rebuilt.function("read_first").expect("rebuilt first")),
            CacheSiteId::new(0)
        );
        assert_eq!(
            load_global_cache_site(rebuilt.function("read_second").expect("rebuilt second")),
            CacheSiteId::new(0)
        );
    }

    fn load_global_cache_site(function: &CodeObject) -> CacheSiteId {
        function
            .instructions
            .iter()
            .find_map(|instruction| match &instruction.kind {
                InstructionKind::LoadGlobal {
                    cache_site: Some(site),
                    ..
                } => Some(*site),
                _ => None,
            })
            .expect("function should have global read cache site")
    }
}
