use crate::type_fact::TypeFact;

mod functions;
mod methods;
mod reflect;

#[cfg(test)]
mod function_tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LambdaFact {
    pub params: Vec<TypeFact>,
    pub returns: TypeFact,
}

impl LambdaFact {
    fn new(params: Vec<TypeFact>, returns: TypeFact) -> Self {
        Self { params, returns }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdlibMethodFact {
    pub receiver: TypeFact,
    pub method: &'static str,
    pub params: Vec<TypeFact>,
    pub lambda: Option<LambdaFact>,
    pub returns: TypeFact,
}

impl StdlibMethodFact {
    fn new(receiver: TypeFact, method: &'static str, returns: TypeFact) -> Self {
        Self {
            receiver,
            method,
            params: Vec::new(),
            lambda: None,
            returns,
        }
    }

    fn with_params(mut self, params: Vec<TypeFact>) -> Self {
        self.params = params;
        self
    }

    fn with_lambda(mut self, params: Vec<TypeFact>, returns: TypeFact) -> Self {
        self.params = vec![TypeFact::function(params.clone(), returns.clone())];
        self.lambda = Some(LambdaFact::new(params, returns));
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdlibFunctionFact {
    pub name: &'static str,
    pub params: Vec<TypeFact>,
    pub returns: TypeFact,
}

impl StdlibFunctionFact {
    fn new(name: &'static str, params: Vec<TypeFact>, returns: TypeFact) -> Self {
        Self {
            name,
            params,
            returns,
        }
    }
}

pub fn stdlib_method_fact(
    receiver: &TypeFact,
    method: &str,
    lambda_return: Option<&TypeFact>,
) -> Option<StdlibMethodFact> {
    methods::method_fact(receiver, method, lambda_return)
}

pub fn stdlib_method_facts(
    receiver: &TypeFact,
    lambda_return: Option<&TypeFact>,
) -> Vec<StdlibMethodFact> {
    methods::method_facts(receiver, lambda_return)
}

pub fn stdlib_function_completion_facts() -> Vec<StdlibFunctionFact> {
    functions::completion_facts()
}

pub fn stdlib_function_fact(name: &str, args: &[TypeFact]) -> Option<StdlibFunctionFact> {
    functions::function_fact(name, args)
}

#[cfg(test)]
mod tests;
