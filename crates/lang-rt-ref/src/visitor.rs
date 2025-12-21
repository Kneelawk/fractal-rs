use crate::visitor::RuntimeError::{FunctionIncorrectNumberOfArgs, FunctionNotFound};
use fractal_rs_3_lang::ExpressionValue;
use fractal_rs_3_lang::ast::visitor::{AstVisitor, VisitResult};
use fractal_rs_3_lang::ast::{
    AstAnnotation, AstBlock, AstExpression, AstFunction, AstIfBlock, AstProgram, AstVariable,
};
use std::borrow::Cow;
use std::collections::HashMap;
use thiserror::Error;

pub struct ReferenceRuntime<'a> {
    program: Cow<'a, AstProgram>,
}

impl<'a> ReferenceRuntime<'a> {
    pub fn new(program: Cow<'a, AstProgram>) -> Self {
        Self { program }
    }

    pub fn run(
        &self,
        globals: HashMap<String, ExpressionValue>,
        fn_name: impl AsRef<str>,
        args: Vec<ExpressionValue>,
    ) -> Result<ExpressionValue, RuntimeError> {
        let mut fn_args = HashMap::new();
        if let Some(fun) = self.program.functions.get(fn_name.as_ref()) {
            if args.len() != fun.args.len() {
                return Err(FunctionIncorrectNumberOfArgs(
                    fn_name.as_ref().to_string(),
                    fun.args.len(),
                    args.len(),
                ));
            }

            for (index, arg) in fun.args.iter().enumerate() {
                fn_args.insert(arg.name.clone(), args[index].clone());
            }
        } else {
            return Err(FunctionNotFound(fn_name.as_ref().to_string()));
        }

        let mut visitor = RuntimeVisitor {
            program: &self.program,
            scopes: vec![globals, fn_args],
        };
        visitor.visit_function(&self.program.functions[fn_name.as_ref()])
    }
}

struct RuntimeVisitor<'a> {
    program: &'a AstProgram,
    scopes: Vec<HashMap<String, ExpressionValue>>,
}

struct ReferenceRuntimeResult;
impl VisitResult for ReferenceRuntimeResult {
    type Program = ();
    type Function = Result<ExpressionValue, RuntimeError>;
    type Expression = Result<ExpressionValue, RuntimeError>;
    type IfBlock = Option<Result<ExpressionValue, RuntimeError>>;
    type Block = Result<ExpressionValue, RuntimeError>;
    type Variable = ();
    type Annotation = ();
}

impl AstVisitor<ReferenceRuntimeResult> for RuntimeVisitor<'_> {
    fn visit_program(&mut self, program: &AstProgram) {
        todo!()
    }

    fn visit_function(&mut self, function: &AstFunction) -> Result<ExpressionValue, RuntimeError> {
        todo!()
    }

    fn visit_expression(
        &mut self,
        expression: &AstExpression,
    ) -> Result<ExpressionValue, RuntimeError> {
        todo!()
    }

    fn visit_if_block(
        &mut self,
        if_block: &AstIfBlock,
    ) -> Option<Result<ExpressionValue, RuntimeError>> {
        todo!()
    }

    fn visit_block(&mut self, block: &AstBlock) -> Result<ExpressionValue, RuntimeError> {
        todo!()
    }

    fn visit_variable(&mut self, variable: &AstVariable) {
        todo!()
    }

    fn visit_annotation(&mut self, annotation: &AstAnnotation) {
        todo!()
    }
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("Function not found: {0}")]
    FunctionNotFound(String),
    #[error("Function {0} incorrect number of args provided: {2}, expected: {1}")]
    FunctionIncorrectNumberOfArgs(String, usize, usize),
}
