use super::QueryContext;
use crate::callable_context::CallableFacts;
use vela_syntax::ast::AstNode;

pub(crate) struct CallArgumentPosition {
    ordinal: usize,
    positional_slots: usize,
    current_name: Option<String>,
    pub(crate) has_equal: bool,
    has_expression: bool,
    prior_names: Vec<String>,
    following_names: Vec<String>,
}

impl QueryContext<'_> {
    pub(crate) fn call_argument_position(&self) -> Option<CallArgumentPosition> {
        let expression = self.syntax_call()?;
        let ordinal = self.call_active_parameter_index()?;
        let separators = expression.separator_tokens();
        let mut result = CallArgumentPosition {
            ordinal,
            positional_slots: ordinal,
            current_name: None,
            has_equal: false,
            has_expression: false,
            prior_names: Vec::new(),
            following_names: Vec::new(),
        };
        for argument in expression.arguments() {
            let index = separators.partition_point(|separator| {
                separator.text_range().end() <= argument.syntax().text_range().start()
            });
            match index.cmp(&ordinal) {
                std::cmp::Ordering::Equal => {
                    result.current_name = argument.name_text();
                    result.has_equal = argument.equal_token().is_some();
                    result.has_expression = argument.expression().is_some();
                }
                std::cmp::Ordering::Less => {
                    if let Some(name) = argument.name_text() {
                        result.positional_slots = result.positional_slots.min(index);
                        result.prior_names.push(name);
                    }
                }
                std::cmp::Ordering::Greater => {
                    if let Some(name) = argument.name_text() {
                        result.following_names.push(name);
                    }
                }
            }
        }
        Some(result)
    }

    pub(crate) fn call_parameter_index(&self, callable: &CallableFacts) -> Option<usize> {
        self.call_argument_position()?.parameter_index(callable)
    }
}

impl CallArgumentPosition {
    fn named_index(callable: &CallableFacts, name: &str) -> Option<usize> {
        callable
            .supports_named_arguments()
            .then(|| {
                callable
                    .params()
                    .iter()
                    .position(|param| param.name() == name)
            })
            .flatten()
    }

    fn occupied(&self, callable: &CallableFacts, following: bool) -> Vec<bool> {
        let mut occupied = vec![false; callable.params().len()];
        // Missing expressions before separators still own their positional slots.
        let positional = self.positional_slots.min(occupied.len());
        occupied[..positional].fill(true);
        for name in self
            .prior_names
            .iter()
            .chain(self.following_names.iter().filter(|_| following))
        {
            if let Some(index) = Self::named_index(callable, name) {
                occupied[index] = true;
            }
        }
        occupied
    }

    pub(crate) fn available_parameters(&self, callable: &CallableFacts) -> Vec<bool> {
        self.occupied(callable, true)
            .into_iter()
            .map(|used| !used)
            .collect()
    }

    pub(crate) fn allows_positional_expression(&self, callable: Option<&CallableFacts>) -> bool {
        if self.has_equal || !self.prior_names.is_empty() {
            return false;
        }
        callable.is_none_or(|callable| {
            self.available_parameters(callable).get(self.ordinal) == Some(&true)
        })
    }

    fn parameter_index(&self, callable: &CallableFacts) -> Option<usize> {
        let occupied = self.occupied(callable, false);
        if let Some(name) = &self.current_name {
            let index = Self::named_index(callable, name)?;
            return (!occupied[index]).then_some(index);
        }
        if !self.has_expression && callable.supports_named_arguments() {
            return self
                .available_parameters(callable)
                .iter()
                .position(|available| *available);
        }
        (self.prior_names.is_empty() && self.ordinal < occupied.len() && !occupied[self.ordinal])
            .then_some(self.ordinal)
    }
}
