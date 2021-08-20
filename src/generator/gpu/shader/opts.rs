use crate::generator::{
    args::{Multisampling, Smoothing, DEFAULT_RADIUS_SQUARED},
    gpu::shader::ShaderError,
    util::FloatKey,
    FractalOpts,
};
use naga::{
    Arena, ArraySize, BinaryOperator, Block, Constant, ConstantInner, Expression, Function, Handle,
    MathFunction, Module, ScalarKind, ScalarValue, Span, Statement, TypeInner, VectorSize,
};
use std::collections::HashMap;

const C_REAL_NAME: &str = "t_c_real";
const C_IMAG_NAME: &str = "t_c_imag";
const ITERATIONS_NAME: &str = "t_iterations";
const MANDELBROT_NAME: &str = "t_mandelbrot";
const RADIUS_SQUARED_NAME: &str = "t_radius_squared";
const SAMPLE_COUNT_NAME: &str = "t_sample_count";
const SAMPLE_OFFSETS_NAME: &str = "t_sample_offsets";
const SMOOTH_NAME: &str = "t_smooth";
const LINEAR_INTERSECTION_NAME: &str = "linear_intersection";

/// Structs implementing this trait can be used when generating fractals on the
/// GPU.
pub trait GpuFractalOpts {
    fn install(&self, module: &mut Module) -> Result<(), ShaderError>;
}

impl GpuFractalOpts for FractalOpts {
    fn install(&self, module: &mut Module) -> Result<(), ShaderError> {
        self.install_c(module)?;
        self.install_iterations(module)?;
        self.install_mandelbrot(module)?;
        self.smoothing.install(module)?;
        self.multisampling.install(module)?;
        Ok(())
    }
}

impl FractalOpts {
    fn install_c(&self, module: &mut Module) -> Result<(), ShaderError> {
        replace_constant(
            &mut module.constants,
            C_REAL_NAME,
            ConstantInner::Scalar {
                width: 4,
                value: ScalarValue::Float(self.c.re as f64),
            },
        )?;

        replace_constant(
            &mut module.constants,
            C_IMAG_NAME,
            ConstantInner::Scalar {
                width: 4,
                value: ScalarValue::Float(self.c.im as f64),
            },
        )?;

        Ok(())
    }

    fn install_iterations(&self, module: &mut Module) -> Result<(), ShaderError> {
        replace_constant(
            &mut module.constants,
            ITERATIONS_NAME,
            ConstantInner::Scalar {
                width: 4,
                value: ScalarValue::Uint(self.iterations as u64),
            },
        )
    }

    fn install_mandelbrot(&self, module: &mut Module) -> Result<(), ShaderError> {
        replace_constant(
            &mut module.constants,
            MANDELBROT_NAME,
            ConstantInner::Scalar {
                width: 1,
                value: ScalarValue::Bool(self.mandelbrot),
            },
        )
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
        replace_constant(
            &mut module.constants,
            RADIUS_SQUARED_NAME,
            ConstantInner::Scalar {
                width: 4,
                value: ScalarValue::Float(self.radius_squared() as f64),
            },
        )?;

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
        let handle = find_function_handle(&module.functions, SMOOTH_NAME)?;
        let linear_intersection_handle =
            find_function_handle(&module.functions, LINEAR_INTERSECTION_NAME)?;

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

/// Structs implementing this trait can be used as multisampling options for
/// generating on the GPU.
pub trait GpuMultisampling {
    fn install(&self, module: &mut Module) -> Result<(), ShaderError>;
}

impl GpuMultisampling for Multisampling {
    fn install(&self, module: &mut Module) -> Result<(), ShaderError> {
        self.install_sample_count(module)?;
        self.install_sample_offsets(module)?;
        Ok(())
    }
}

impl Multisampling {
    fn install_sample_count(&self, module: &mut Module) -> Result<(), ShaderError> {
        replace_constant(
            &mut module.constants,
            SAMPLE_COUNT_NAME,
            ConstantInner::Scalar {
                width: 4,
                value: ScalarValue::Uint(self.sample_count() as u64),
            },
        )
    }

    fn install_sample_offsets(&self, module: &mut Module) -> Result<(), ShaderError> {
        let sample_count_handle = find_constant(&module.constants, SAMPLE_COUNT_NAME)?;
        let vec2_f32_type_handle = module
            .types
            .fetch_if(|t| {
                t.inner
                    == TypeInner::Vector {
                        size: VectorSize::Bi,
                        kind: ScalarKind::Float,
                        width: 4,
                    }
            })
            .ok_or_else(|| ShaderError::MissingTemplateType("vec2<f32>".to_string()))?;
        let sample_count_type_handle = module
            .types
            .fetch_if(|t| {
                t.inner
                    == TypeInner::Array {
                        base: vec2_f32_type_handle,
                        size: ArraySize::Constant(sample_count_handle),
                        stride: 8,
                    }
            })
            .ok_or_else(|| {
                ShaderError::MissingTemplateType("array<vec2<f32>, t_sample_count>".to_string())
            })?;

        let handle = find_function_handle(&module.functions, SAMPLE_OFFSETS_NAME)?;

        let function = module.functions.get_mut(handle);
        function.body = Block::new();
        function.expressions.clear();
        function.local_variables.clear();
        function.named_expressions.clear();

        let mut constants = HashMap::new();

        let offsets = self.offsets();
        for offset in offsets.iter() {
            let x_key = FloatKey::from_f32(offset.x);
            let y_key = FloatKey::from_f32(offset.y);

            if !constants.contains_key(&x_key) {
                constants.insert(
                    x_key,
                    function.expressions.append(
                        Expression::Constant(get_float_constant(&mut module.constants, offset.x)),
                        Span::Unknown,
                    ),
                );
            }

            if !constants.contains_key(&y_key) {
                constants.insert(
                    y_key,
                    function.expressions.append(
                        Expression::Constant(get_float_constant(&mut module.constants, offset.y)),
                        Span::Unknown,
                    ),
                );
            }
        }

        let compose_start = function.expressions.len();
        let mut vec_handles = vec![];
        for offset in offsets {
            let x_key = FloatKey::from_f32(offset.x);
            let y_key = FloatKey::from_f32(offset.y);
            vec_handles.push(function.expressions.append(
                Expression::Compose {
                    ty: vec2_f32_type_handle,
                    components: vec![constants[&x_key], constants[&y_key]],
                },
                Span::Unknown,
            ));
        }

        let final_handle = function.expressions.append(
            Expression::Compose {
                ty: sample_count_type_handle,
                components: vec_handles,
            },
            Span::Unknown,
        );
        let compose_range = function.expressions.range_from(compose_start);

        function
            .body
            .push(Statement::Emit(compose_range), Span::Unknown);
        function.body.push(
            Statement::Return {
                value: Some(final_handle),
            },
            Span::Unknown,
        );

        Ok(())
    }
}

fn find_function_handle(
    functions: &Arena<Function>,
    name: &str,
) -> Result<Handle<Function>, ShaderError> {
    functions
        .fetch_if(|f| match f.name {
            None => false,
            Some(ref fname) => fname == name,
        })
        .ok_or_else(|| ShaderError::MissingTemplateFunction(name.to_string()))
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

fn replace_constant(
    constants: &mut Arena<Constant>,
    name: &str,
    inner: ConstantInner,
) -> Result<(), ShaderError> {
    let handle = find_constant(constants, name)?;
    let constant = constants.get_mut(handle);

    constant.inner = inner;

    Ok(())
}

fn find_constant(constants: &Arena<Constant>, name: &str) -> Result<Handle<Constant>, ShaderError> {
    constants
        .fetch_if(|c| match c.name {
            None => false,
            Some(ref const_name) => const_name == name,
        })
        .ok_or_else(|| ShaderError::MissingTemplateConstant(name.to_string()))
}
