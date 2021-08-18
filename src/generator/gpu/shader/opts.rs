use crate::generator::{
    args::{Smoothing, DEFAULT_RADIUS_SQUARED},
    gpu::shader::ShaderError,
    FractalOpts,
};
use naga::{
    Arena, BinaryOperator, Block, Constant, ConstantInner, Expression, Function, Handle,
    MathFunction, Module, ScalarKind, ScalarValue, Span, Statement,
};

const RADIUS_SQUARED_NAME: &str = "radius_squared";
const SMOOTH_NAME: &str = "smooth";
const LINEAR_INTERSECTION_NAME: &str = "linear_intersection";

/// Structs implementing this trait can be used when generating fractals on the
/// GPU.
pub trait GpuFractalOpts {
    fn install(&self, module: &mut Module) -> Result<(), ShaderError>;
}

impl GpuFractalOpts for FractalOpts {
    fn install(&self, module: &mut Module) -> Result<(), ShaderError> {
        self.smoothing.install(module)?;
        Ok(())
    }
}

/// Structs implementing this trait can be used as smoothing options for
/// generating fractals on the GPU.
pub trait GpuSmoothing {
    fn install(&self, module: &mut Module) -> Result<(), ShaderError>;
}

impl GpuSmoothing for Smoothing {
    fn install(&self, module: &mut Module) -> Result<(), ShaderError> {
        self.install_radius_squared(module)?;
        self.install_smoothing(module)?;
        Ok(())
    }
}

impl Smoothing {
    fn install_radius_squared(&self, module: &mut Module) -> Result<(), ShaderError> {
        let handle = module
            .constants
            .fetch_if(|c| match c.name {
                None => false,
                Some(ref name) => name == RADIUS_SQUARED_NAME,
            })
            .ok_or_else(|| ShaderError::MissingTemplateConstant(RADIUS_SQUARED_NAME.to_string()))?;

        let constant = module.constants.get_mut(handle);

        constant.inner = ConstantInner::Scalar {
            width: 4,
            value: ScalarValue::Float(self.radius_squared() as f64),
        };

        Ok(())
    }

    fn radius_squared(&self) -> f32 {
        match self {
            Smoothing::None => DEFAULT_RADIUS_SQUARED,
            Smoothing::LogarithmicDistance { radius_squared, .. } => *radius_squared,
            Smoothing::LinearIntersection => DEFAULT_RADIUS_SQUARED,
        }
    }

    fn install_smoothing(&self, module: &mut Module) -> Result<(), ShaderError> {
        let handle = find_function_handle(module, SMOOTH_NAME)?;
        let linear_intersection_handle = find_function_handle(module, LINEAR_INTERSECTION_NAME)?;

        let function = module.functions.get_mut(handle);
        function.body = Block::new();
        function.expressions.clear();
        function.local_variables.clear();
        function.named_expressions.clear();

        let iterations_handle = function
            .expressions
            .append(Expression::FunctionArgument(0), Span::Unknown);
        let z_curr_handle = function
            .expressions
            .append(Expression::FunctionArgument(1), Span::Unknown);
        let z_prev_handle = function
            .expressions
            .append(Expression::FunctionArgument(2), Span::Unknown);

        match self {
            Smoothing::None => {
                let cast_handle_start = function.expressions.len();
                let cast_handle = function.expressions.append(
                    Expression::As {
                        expr: iterations_handle,
                        kind: ScalarKind::Float,
                        convert: Some(4),
                    },
                    Span::Unknown,
                );
                let cast_handle_range = function.expressions.range_from(cast_handle_start);

                function
                    .body
                    .extend(Some((Statement::Emit(cast_handle_range), Span::Unknown)));

                function.body.extend(Some((
                    Statement::Return {
                        value: Some(cast_handle),
                    },
                    Span::Unknown,
                )));
            },
            Smoothing::LogarithmicDistance {
                divisor, addend, ..
            } => {
                let divisor_constant = get_float_constant(&mut module.constants, *divisor);
                let addend_constant = get_float_constant(&mut module.constants, *addend);

                let divisor_handle = function
                    .expressions
                    .append(Expression::Constant(divisor_constant), Span::Unknown);
                let addend_handle = function
                    .expressions
                    .append(Expression::Constant(addend_constant), Span::Unknown);

                let range_start = function.expressions.len();
                let cast_handle = function.expressions.append(
                    Expression::As {
                        expr: iterations_handle,
                        kind: ScalarKind::Float,
                        convert: Some(4),
                    },
                    Span::Unknown,
                );
                let length_handle = function.expressions.append(
                    Expression::Math {
                        fun: MathFunction::Dot,
                        arg: z_curr_handle,
                        arg1: Some(z_curr_handle),
                        arg2: None,
                    },
                    Span::Unknown,
                );
                let log0_handle = function.expressions.append(
                    Expression::Math {
                        fun: MathFunction::Log,
                        arg: length_handle,
                        arg1: None,
                        arg2: None,
                    },
                    Span::Unknown,
                );
                let log1_handle = function.expressions.append(
                    Expression::Math {
                        fun: MathFunction::Log,
                        arg: log0_handle,
                        arg1: None,
                        arg2: None,
                    },
                    Span::Unknown,
                );
                let divide_handle = function.expressions.append(
                    Expression::Binary {
                        op: BinaryOperator::Divide,
                        left: log1_handle,
                        right: divisor_handle,
                    },
                    Span::Unknown,
                );
                let subtract_handle = function.expressions.append(
                    Expression::Binary {
                        op: BinaryOperator::Subtract,
                        left: cast_handle,
                        right: divide_handle,
                    },
                    Span::Unknown,
                );
                let add_handle = function.expressions.append(
                    Expression::Binary {
                        op: BinaryOperator::Add,
                        left: subtract_handle,
                        right: addend_handle,
                    },
                    Span::Unknown,
                );
                let range = function.expressions.range_from(range_start);

                function
                    .body
                    .extend(Some((Statement::Emit(range), Span::Unknown)));
                function.body.extend(Some((
                    Statement::Return {
                        value: Some(add_handle),
                    },
                    Span::Unknown,
                )));
            },
            Smoothing::LinearIntersection => {
                let linear_intersection_call_handle = function.expressions.append(
                    Expression::CallResult(linear_intersection_handle),
                    Span::Unknown,
                );

                function.body.extend(Some((
                    Statement::Call {
                        function: linear_intersection_handle,
                        arguments: vec![iterations_handle, z_curr_handle, z_prev_handle],
                        result: Some(linear_intersection_call_handle),
                    },
                    Span::Unknown,
                )));
                function.body.extend(Some((
                    Statement::Return {
                        value: Some(linear_intersection_call_handle),
                    },
                    Span::Unknown,
                )));
            },
        }

        Ok(())
    }
}

fn find_function_handle(module: &Module, name: &str) -> Result<Handle<Function>, ShaderError> {
    Ok(module
        .functions
        .fetch_if(|f| match f.name {
            None => false,
            Some(ref fname) => fname == name,
        })
        .ok_or_else(|| ShaderError::MissingTemplateFunction(name.to_string()))?)
}

fn get_float_constant(constants: &mut Arena<Constant>, value: f32) -> Handle<Constant> {
    constants.fetch_or_append(
        Constant {
            name: None,
            specialization: None,
            inner: ConstantInner::Scalar {
                width: 4,
                value: ScalarValue::Float(value as f64),
            },
        },
        Span::Unknown,
    )
}
