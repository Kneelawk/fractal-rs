use crate::ast::{
    AstAnnotation, AstBlock, AstExpression, AstFunction, AstIfBlock, AstProgram, AstVariable,
};

/// The result of an ast visit or mutation
pub trait VisitResult {
    type Program;
    type Function;
    type Expression;
    type IfBlock;
    type Block;
    type Variable;
    type Annotation;
}

/// A visitor that can walk an ast without mutating it.
pub trait AstVisitor<T: VisitResult> {
    fn visit_program(&mut self, program: &AstProgram) -> T::Program;

    fn visit_function(&mut self, function: &AstFunction) -> T::Function;

    fn visit_expression(&mut self, expression: &AstExpression) -> T::Expression;

    fn visit_if_block(&mut self, if_block: &AstIfBlock) -> T::IfBlock;

    fn visit_block(&mut self, block: &AstBlock) -> T::Block;

    fn visit_variable(&mut self, variable: &AstVariable) -> T::Variable;

    fn visit_annotation(&mut self, variable: &AstAnnotation) -> T::Annotation;
}

/// A visitor that mutates an ast as it walks it
pub trait AstMutator<T: VisitResult> {
    fn visit_program(&mut self, program: &mut AstProgram) -> T::Program;

    fn visit_function(&mut self, function: &mut AstFunction) -> T::Function;

    fn visit_expression(&mut self, expression: &mut AstExpression) -> T::Expression;

    fn visit_if_block(&mut self, if_block: &mut AstIfBlock) -> T::IfBlock;

    fn visit_block(&mut self, block: &mut AstBlock) -> T::Block;

    fn visit_variable(&mut self, variable: &mut AstVariable) -> T::Variable;

    fn visit_annotation(&mut self, variable: &mut AstAnnotation) -> T::Annotation;
}

impl VisitResult for () {
    type Program = ();
    type Function = ();
    type Expression = ();
    type IfBlock = ();
    type Block = ();
    type Variable = ();
    type Annotation = ();
}
