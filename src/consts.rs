use crate::compiler::CompileError;
use crate::types::{FloatType, IntType};
use crate::utils::success;
use num_traits::Float;
use std::hash::{Hash, Hasher};

#[derive(Clone, PartialEq)]
pub enum Const {
    Int(IntType),
    Float(FloatType),
    Str(String),
}

impl Eq for Const {}

impl Hash for Const {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Const::Int(i) => i.hash(state),
            Const::Float(f) => {
                let (m, e, s) = Float::integer_decode(*f);
                m.hash(state);
                e.hash(state);
                s.hash(state);
            }
            Const::Str(s) => s.hash(state),
        }
    }
}

fn float_to_int(f: FloatType) -> Option<IntType> {
    if f.floor() == f {
        Some(f as IntType)
    } else {
        None
    }
}

fn ignore_unhashable_float(
    input: Result<Option<Const>, CompileError>,
) -> Result<Option<Const>, CompileError> {
    // compatible with lua
    // won't constant folding Nan/Inf/0.0
    match &input {
        Ok(Some(Const::Float(f))) if (*f).is_nan() || (*f).is_infinite() || *f == 0.0 => Ok(None),
        _ => input,
    }
}

macro_rules! bin_op {
    ($name:ident, $int_int:expr, $int_float:expr, $float_int:expr, $float_float:expr $(, $access:tt)?) => {
        $($access)? fn $name(self, other: Const) -> Result<Option<Const>, CompileError> {
            let result = match self {
                Const::Int(a) => match other {
                    Const::Int(b) => $int_int(a, b),
                    Const::Float(b) => $int_float(a, b),
                    _ => unreachable!(),
                },
                Const::Float(a) => match other {
                    Const::Int(b) => $float_int(a, b),
                    Const::Float(b) => $float_float(a, b),
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            };

            ignore_unhashable_float(result)
        }
    };
}

macro_rules! bin_op_normal {
    ($name:ident, $op:tt) => {
        bin_op! {
            $name,
            |a, b| success(Const::Int(a $op b)),
            |a, b| success(Const::Float(a as FloatType $op b)),
            |a, b| success(Const::Float(a $op b as FloatType)),
            |a, b| success(Const::Float(a $op b))
        }
    };
}

macro_rules! bin_op_int {
    ($name:ident, $op:tt) => {
        bin_op! {
            $name,
            |a, b| success(Const::Int(a $op b)),
            |a, b| Ok(float_to_int(b).map(|b| Const::Int(a $op b))),
            |a, b| Ok(float_to_int(a).map(|a| Const::Int(a $op b))),
            |a, b| Ok(float_to_int(a).and_then(|a| float_to_int(b).and_then(|b| Some(Const::Int(a $op b)))))
        }
    };
}

impl Const {
    bin_op! {
        idiv,
        |a, b| if b == 0 { Err(CompileError::new("divide by zero")) } else { success(Const::Int(a / b)) },
        |_, _| Ok(None),
        |_, _| Ok(None),
        |_, _| Ok(None),
        pub
    }

    bin_op! {
        pow,
        |a, b| success(Const::Float((a as FloatType).powf(b as FloatType))),
        |a, b| success(Const::Float((a as FloatType).powf(b))),
        |a:FloatType, b| success(Const::Float(a.powf(b as FloatType))),
        |a:FloatType, b| success(Const::Float(a.powf(b))),
        pub
    }

    pub fn minus(&self) -> Result<Option<Const>, CompileError> {
        let result = match self {
            Const::Int(i) => success(Const::Int(-i)),
            Const::Float(f) => success(Const::Float(-f)),
            _ => return Ok(None),
        };
        ignore_unhashable_float(result)
    }

    pub fn bnot(&self) -> Result<Option<Const>, CompileError> {
        match self {
            Const::Int(i) => success(Const::Int(!i)),
            _ => return Ok(None),
        }
    }
}

impl std::ops::Add for Const {
    type Output = Result<Option<Const>, CompileError>;
    bin_op_normal! {add, +}
}

impl std::ops::Sub for Const {
    type Output = Result<Option<Const>, CompileError>;
    bin_op_normal! {sub, -}
}

impl std::ops::Mul for Const {
    type Output = Result<Option<Const>, CompileError>;
    bin_op_normal! {mul, *}
}

impl std::ops::Div for Const {
    type Output = Result<Option<Const>, CompileError>;
    bin_op! {
        div,
        |a, b| success(Const::Float(a as FloatType / b as FloatType)),
        |a, b| success(Const::Float(a as FloatType / b)),
        |a, b| success(Const::Float(a / b as FloatType)),
        |a, b| success(Const::Float(a / b))
    }
}

impl std::ops::Rem for Const {
    type Output = Result<Option<Const>, CompileError>;
    bin_op! {
        rem,
        |a, b| success(Const::Int(a % b)),
        |a, b| success(Const::Float(a as FloatType % b)),
        |a, b| success(Const::Float(a % b as FloatType)),
        |a, b| success(Const::Float(a % b))
    }
}

impl std::ops::BitXor for Const {
    type Output = Result<Option<Const>, CompileError>;
    bin_op_int! {bitxor, ^}
}

impl std::ops::BitAnd for Const {
    type Output = Result<Option<Const>, CompileError>;
    bin_op_int! {bitand, &}
}

impl std::ops::BitOr for Const {
    type Output = Result<Option<Const>, CompileError>;
    bin_op_int! {bitor, |}
}

impl std::ops::Shl for Const {
    type Output = Result<Option<Const>, CompileError>;
    bin_op_int! {shl, <<}
}

impl std::ops::Shr for Const {
    type Output = Result<Option<Const>, CompileError>;
    bin_op_int! {shr, >>}
}
