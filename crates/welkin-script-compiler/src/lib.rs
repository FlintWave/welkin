// SPDX-License-Identifier: GPL-3.0-or-later
//! LSL type system and compiler types for the OpenSimulator script engine.
//!
//! This crate defines the LSL type system used by both the compiler and runtime.

pub mod ast;
pub mod lexer;
pub mod parser;

use std::fmt;

/// The primitive types available in LSL (Linden Scripting Language).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LslType {
    /// 32-bit signed integer
    Integer,
    /// 32-bit IEEE 754 float (LSL uses single precision semantics)
    Float,
    /// UTF-8 string
    String,
    /// UUID key type
    Key,
    /// 3-component vector (x, y, z) of floats
    Vector,
    /// 4-component rotation quaternion (x, y, z, s) of floats
    Rotation,
    /// Heterogeneous list of LSL values
    List,
    /// No return value (used for function return types)
    Void,
}

impl fmt::Display for LslType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer => write!(f, "integer"),
            Self::Float => write!(f, "float"),
            Self::String => write!(f, "string"),
            Self::Key => write!(f, "key"),
            Self::Vector => write!(f, "vector"),
            Self::Rotation => write!(f, "rotation"),
            Self::List => write!(f, "list"),
            Self::Void => write!(f, "void"),
        }
    }
}

/// A concrete LSL runtime value.
#[derive(Debug, Clone, PartialEq)]
pub enum LslValue {
    Integer(i32),
    Float(f32),
    String(std::string::String),
    Key(std::string::String),
    Vector { x: f32, y: f32, z: f32 },
    Rotation { x: f32, y: f32, z: f32, s: f32 },
    List(Vec<LslValue>),
}

impl LslValue {
    /// Returns the [`LslType`] of this value.
    pub fn lsl_type(&self) -> LslType {
        match self {
            Self::Integer(_) => LslType::Integer,
            Self::Float(_) => LslType::Float,
            Self::String(_) => LslType::String,
            Self::Key(_) => LslType::Key,
            Self::Vector { .. } => LslType::Vector,
            Self::Rotation { .. } => LslType::Rotation,
            Self::List(_) => LslType::List,
        }
    }

    /// Returns the default (zero) value for the given LSL type.
    ///
    /// # Panics
    /// Panics if `ty` is `Void`.
    pub fn default_for(ty: LslType) -> Self {
        match ty {
            LslType::Integer => Self::Integer(0),
            LslType::Float => Self::Float(0.0),
            LslType::String => Self::String(std::string::String::new()),
            LslType::Key => Self::Key("00000000-0000-0000-0000-000000000000".into()),
            LslType::Vector => Self::Vector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            LslType::Rotation => Self::Rotation {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                s: 1.0,
            },
            LslType::List => Self::List(Vec::new()),
            LslType::Void => panic!("no default value for Void"),
        }
    }
}

impl fmt::Display for LslValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v:.6}"),
            Self::String(v) | Self::Key(v) => write!(f, "{v}"),
            Self::Vector { x, y, z } => write!(f, "<{x:.6}, {y:.6}, {z:.6}>"),
            Self::Rotation { x, y, z, s } => {
                write!(f, "<{x:.6}, {y:.6}, {z:.6}, {s:.6}>")
            }
            Self::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_display() {
        assert_eq!(LslType::Integer.to_string(), "integer");
        assert_eq!(LslType::Float.to_string(), "float");
        assert_eq!(LslType::String.to_string(), "string");
        assert_eq!(LslType::Key.to_string(), "key");
        assert_eq!(LslType::Vector.to_string(), "vector");
        assert_eq!(LslType::Rotation.to_string(), "rotation");
        assert_eq!(LslType::List.to_string(), "list");
        assert_eq!(LslType::Void.to_string(), "void");
    }

    #[test]
    fn value_type_round_trip() {
        assert_eq!(LslValue::Integer(42).lsl_type(), LslType::Integer);
        #[allow(clippy::approx_constant)]
        assert_eq!(LslValue::Float(3.14).lsl_type(), LslType::Float);
        assert_eq!(LslValue::String("hello".into()).lsl_type(), LslType::String);
        assert_eq!(
            LslValue::Key("00000000-0000-0000-0000-000000000000".into()).lsl_type(),
            LslType::Key
        );
        assert_eq!(
            LslValue::Vector {
                x: 1.0,
                y: 2.0,
                z: 3.0
            }
            .lsl_type(),
            LslType::Vector
        );
        assert_eq!(
            LslValue::Rotation {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                s: 1.0
            }
            .lsl_type(),
            LslType::Rotation
        );
        assert_eq!(LslValue::List(vec![]).lsl_type(), LslType::List);
    }

    #[test]
    fn default_values() {
        assert_eq!(
            LslValue::default_for(LslType::Integer),
            LslValue::Integer(0)
        );
        assert_eq!(LslValue::default_for(LslType::Float), LslValue::Float(0.0));
        assert_eq!(
            LslValue::default_for(LslType::Rotation),
            LslValue::Rotation {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                s: 1.0
            }
        );
    }

    #[test]
    fn value_display() {
        assert_eq!(LslValue::Integer(42).to_string(), "42");
        assert_eq!(
            LslValue::Vector {
                x: 1.0,
                y: 2.0,
                z: 3.0
            }
            .to_string(),
            "<1.000000, 2.000000, 3.000000>"
        );
    }
}
