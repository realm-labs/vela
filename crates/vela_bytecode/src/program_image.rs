use std::collections::BTreeMap;

use vela_common::StateSlot;
use vela_def::FunctionId;
use vela_hir::module_graph::ModuleGraph;

use crate::script_methods::ScriptMethodTable;
use crate::{
    CacheSiteDesc, CacheSiteId, CacheSiteInstruction, CacheSiteLayout, FunctionIndex,
    UnlinkedCodeObject, UnlinkedInstructionKind, UnlinkedProgram, UnlinkedProgramCode,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ProgramImage {
    functions: Box<[UnlinkedCodeObject]>,
    function_by_name: BTreeMap<String, FunctionIndex>,
    function_by_id: BTreeMap<FunctionId, FunctionIndex>,
    global_names: Box<[String]>,
    global_slots: BTreeMap<String, StateSlot>,
    cache_sites: Box<[CacheSiteDesc]>,
    script_methods: ScriptMethodTable,
    script_metadata: Option<ModuleGraph>,
}

impl ProgramImage {
    #[must_use]
    pub(crate) fn from_program(program: &UnlinkedProgram) -> Self {
        Self::from_parts(
            program.functions().cloned(),
            program.global_names().iter().cloned(),
            program.script_methods().clone(),
            program.script_metadata().cloned(),
        )
    }

    #[must_use]
    pub(crate) fn from_parts(
        functions: impl IntoIterator<Item = UnlinkedCodeObject>,
        global_names: impl IntoIterator<Item = String>,
        script_methods: ScriptMethodTable,
        script_metadata: Option<ModuleGraph>,
    ) -> Self {
        let functions = functions.into_iter().collect::<Vec<_>>();
        let mut indexed_functions = Vec::with_capacity(functions.len());
        let mut nested_functions = Vec::new();
        let mut function_by_name = BTreeMap::new();
        let mut function_by_id = BTreeMap::new();
        for function in functions {
            let name = function.name.clone();
            let index = FunctionIndex(indexed_functions.len());
            if let Some(identity) = function.compiled_mir {
                function_by_id.insert(identity.root, index);
            }
            function_by_name.insert(name, index);
            indexed_functions.push(function);
        }
        let top_level_count = indexed_functions.len();
        for function in indexed_functions.iter_mut().take(top_level_count) {
            let nested = std::mem::take(&mut function.nested_functions);
            let remapped = flatten_nested_functions(nested, top_level_count, &mut nested_functions);
            rewrite_closure_function_indices(function, &remapped);
        }
        indexed_functions.extend(nested_functions);
        let cache_sites = rewrite_image_cache_sites(&mut indexed_functions);

        let global_names = global_names.into_iter().collect::<Vec<_>>();
        let global_slots = global_names
            .iter()
            .enumerate()
            .map(|(slot, name)| (name.clone(), StateSlot::new(slot)))
            .collect();

        Self {
            functions: indexed_functions.into_boxed_slice(),
            function_by_name,
            function_by_id,
            global_names: global_names.into_boxed_slice(),
            global_slots,
            cache_sites,
            script_methods,
            script_metadata,
        }
    }

    #[must_use]
    pub fn function(&self, index: FunctionIndex) -> Option<&UnlinkedCodeObject> {
        self.functions.get(index.0)
    }

    #[must_use]
    pub fn function_by_name(&self, name: &str) -> Option<&UnlinkedCodeObject> {
        self.function(self.function_index(name)?)
    }

    #[must_use]
    pub fn function_by_id(&self, id: FunctionId) -> Option<&UnlinkedCodeObject> {
        self.function(self.function_by_id.get(&id).copied()?)
    }

    #[must_use]
    pub fn function_index(&self, name: &str) -> Option<FunctionIndex> {
        self.function_by_name.get(name).copied()
    }

    pub fn functions(&self) -> impl Iterator<Item = (FunctionIndex, &UnlinkedCodeObject)> {
        self.functions
            .iter()
            .enumerate()
            .map(|(index, function)| (FunctionIndex(index), function))
    }

    pub fn entry_function_names(&self) -> impl Iterator<Item = &str> {
        self.function_by_name.keys().map(String::as_str)
    }

    pub fn entry_function_ids(&self) -> impl Iterator<Item = (FunctionId, FunctionIndex)> + '_ {
        self.function_by_id.iter().map(|(id, index)| (*id, *index))
    }

    #[must_use]
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    #[must_use]
    pub fn global_slot(&self, name: &str) -> Option<StateSlot> {
        self.global_slots.get(name).copied()
    }

    #[must_use]
    pub fn global_name(&self, slot: StateSlot) -> Option<&str> {
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

    #[must_use]
    pub fn cache_sites(&self) -> &[CacheSiteDesc] {
        &self.cache_sites
    }

    pub fn verify(&self) -> Result<(), crate::verification::VerificationError> {
        crate::verification::verify_program_image(self)
    }
}

impl UnlinkedProgramCode for ProgramImage {
    fn script_metadata(&self) -> Option<&ModuleGraph> {
        ProgramImage::script_metadata(self)
    }

    fn function(&self, name: &str) -> Option<&UnlinkedCodeObject> {
        self.function_by_name(name)
    }

    fn function_by_index(&self, index: FunctionIndex) -> Option<&UnlinkedCodeObject> {
        self.function(index)
    }

    fn function_by_id(&self, id: FunctionId) -> Option<&UnlinkedCodeObject> {
        ProgramImage::function_by_id(self, id)
    }

    fn script_method(&self, owner: vela_def::TypeId, method: &str) -> Option<&UnlinkedCodeObject> {
        let method = self.script_methods.get(owner, method)?;
        self.function_by_id(method.function_id)
            .or_else(|| self.function_by_name(&method.function))
    }

    fn script_method_id(
        &self,
        owner: vela_def::TypeId,
        method: &str,
    ) -> Option<vela_def::MethodId> {
        self.script_methods
            .get(owner, method)
            .map(|method| method.id)
    }

    fn script_method_by_id(
        &self,
        owner: vela_def::TypeId,
        method_id: vela_def::MethodId,
    ) -> Option<&UnlinkedCodeObject> {
        let method = self.script_methods.get_by_id(owner, method_id)?;
        self.function_by_id(method.function_id)
            .or_else(|| self.function_by_name(&method.function))
    }
}

fn flatten_nested_functions(
    functions: Vec<UnlinkedCodeObject>,
    top_level_count: usize,
    flattened: &mut Vec<UnlinkedCodeObject>,
) -> Vec<FunctionIndex> {
    let mut remapped = Vec::with_capacity(functions.len());
    for mut function in functions {
        let nested = std::mem::take(&mut function.nested_functions);
        let nested = flatten_nested_functions(nested, top_level_count, flattened);
        rewrite_closure_function_indices(&mut function, &nested);
        let index = FunctionIndex(top_level_count + flattened.len());
        flattened.push(function);
        remapped.push(index);
    }
    remapped
}

fn rewrite_closure_function_indices(function: &mut UnlinkedCodeObject, remapped: &[FunctionIndex]) {
    for instruction in &mut function.instructions {
        if let UnlinkedInstructionKind::MakeClosure { function, .. } = &mut instruction.kind
            && let Some(index) = remapped.get(function.0)
        {
            *function = *index;
        }
    }
}

fn rewrite_image_cache_sites(functions: &mut [UnlinkedCodeObject]) -> Box<[CacheSiteDesc]> {
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

fn rewrite_instruction_cache_sites(
    function: &mut UnlinkedCodeObject,
    remapped: &[Option<CacheSiteId>],
) {
    for instruction in &mut function.instructions {
        if let Some(site) = instruction.kind.cache_site()
            && let Some(Some(remapped)) = remapped.get(site.index())
        {
            instruction.kind.set_cache_site(*remapped);
        }
    }
}

#[cfg(test)]
mod tests {
    use vela_common::{HostTypeId, StateSlot};
    use vela_def::{FieldId, FunctionId, MethodId, TypeId};
    use vela_host::target::HostTargetPlan;

    use crate::{
        CacheSiteId, CacheSiteKind, Constant, InstructionOffset, Register, UnlinkedCodeObject,
        UnlinkedInstruction, UnlinkedInstructionKind, UnlinkedProgram,
    };

    use super::ProgramImage;

    #[test]
    fn image_indexes_functions_by_stable_names() {
        let mut program = UnlinkedProgram::new();
        program.insert_function(UnlinkedCodeObject::new("zeta", 0));
        program.insert_function(UnlinkedCodeObject::new("alpha", 0));

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
        let mut program = UnlinkedProgram::new();
        program.set_global_layout(["main::first".to_owned(), "main::second".to_owned()]);
        program.insert_function(UnlinkedCodeObject::new("main", 0));
        let owner = TypeId::new(11);
        program.insert_script_method(
            owner,
            "Player",
            "bonus",
            MethodId::new(7),
            FunctionId::new(8),
            "main",
        );

        let image = ProgramImage::from_program(&program);

        assert_eq!(image.global_slot("main::first"), Some(StateSlot::new(0)));
        assert_eq!(image.global_name(StateSlot::new(1)), Some("main::second"));
        assert_eq!(image.global_names(), program.global_names());
        assert_eq!(
            image
                .script_methods()
                .get_by_id(owner, MethodId::new(7))
                .map(|method| method.function.as_str()),
            Some("main")
        );
    }

    #[test]
    fn image_is_detached_from_later_program_mutation() {
        let mut program = UnlinkedProgram::new();
        let mut main = UnlinkedCodeObject::new("main", 0);
        main.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(1)));
        program.insert_function(main);

        let image = ProgramImage::from_program(&program);
        program
            .function_mut("main")
            .expect("main function")
            .push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));

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
        let mut program = UnlinkedProgram::new();
        let mut main = UnlinkedCodeObject::new("main", 1);
        let closure = UnlinkedCodeObject::new("main::<lambda>", 1);
        let local_function = main.push_nested_function(closure);
        main.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::MakeClosure {
                dst: Register(0),
                function: local_function,
                captures: Vec::new(),
            },
        ));
        program.insert_function(main);

        let image = ProgramImage::from_program(&program);
        let main_index = image.function_index("main").expect("main function index");
        let main = image.function(main_index).expect("main function");
        let closure_index = match &main.instructions[0].kind {
            UnlinkedInstructionKind::MakeClosure { function, .. } => *function,
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
    fn image_rewrites_cache_site_ids_to_image_global_indexes() {
        let mut first = UnlinkedCodeObject::new("read_first", 1);
        let first_local = first.push_cache_site(CacheSiteKind::GlobalRead, InstructionOffset(0));
        first.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::LoadGlobal {
                dst: Register(0),
                global: "main::first".to_owned(),
                slot: None,
                cache_site: Some(first_local),
            },
        ));
        let mut second = UnlinkedCodeObject::new("read_second", 1);
        let second_local = second.push_cache_site(CacheSiteKind::GlobalRead, InstructionOffset(0));
        second.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::LoadGlobal {
                dst: Register(0),
                global: "main::second".to_owned(),
                slot: None,
                cache_site: Some(second_local),
            },
        ));
        assert_eq!(first_local, second_local);

        let mut program = UnlinkedProgram::new();
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
    }

    #[test]
    fn image_rewrites_host_cache_site_ids_to_image_global_indexes() {
        let mut first = UnlinkedCodeObject::new("read_first_host", 2);
        let first_target = first
            .intern_host_target(HostTargetPlan::new(HostTypeId::new(1)).field(FieldId::new(1)));
        let first_local = first.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
        first.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::HostRead {
                dst: Register(1),
                root: Register(0),
                target: first_target,
                dynamic_args: Vec::new(),
                cache_site: first_local,
            },
        ));

        let mut second = UnlinkedCodeObject::new("read_second_host", 2);
        let second_target = second
            .intern_host_target(HostTargetPlan::new(HostTypeId::new(1)).field(FieldId::new(1)));
        let second_local =
            second.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
        second.push_instruction(UnlinkedInstruction::new(
            UnlinkedInstructionKind::HostRead {
                dst: Register(1),
                root: Register(0),
                target: second_target,
                dynamic_args: Vec::new(),
                cache_site: second_local,
            },
        ));
        assert_eq!(first_local, second_local);

        let mut program = UnlinkedProgram::new();
        program.insert_function(first);
        program.insert_function(second);
        let image = ProgramImage::from_program(&program);
        let first = image
            .function_by_name("read_first_host")
            .expect("first host function");
        let second = image
            .function_by_name("read_second_host")
            .expect("second host function");
        let first_site = host_read_cache_site(first);
        let second_site = host_read_cache_site(second);

        assert_eq!(image.cache_site_count(), 2);
        assert_ne!(first_site, second_site);
        assert_eq!(
            image.cache_site(first_site).expect("first host site").id,
            first_site
        );
        assert_eq!(
            image.cache_site(second_site).expect("second host site").id,
            second_site
        );
        assert_eq!(image.verify(), Ok(()));
    }

    fn load_global_cache_site(function: &UnlinkedCodeObject) -> CacheSiteId {
        function
            .instructions
            .iter()
            .find_map(|instruction| match &instruction.kind {
                UnlinkedInstructionKind::LoadGlobal {
                    cache_site: Some(site),
                    ..
                } => Some(*site),
                _ => None,
            })
            .expect("function should have global read cache site")
    }

    fn host_read_cache_site(function: &UnlinkedCodeObject) -> CacheSiteId {
        function
            .instructions
            .iter()
            .find_map(|instruction| match &instruction.kind {
                UnlinkedInstructionKind::HostRead { cache_site, .. } => Some(*cache_site),
                _ => None,
            })
            .expect("function should have host read cache site")
    }
}
